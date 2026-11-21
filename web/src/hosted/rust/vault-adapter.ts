import { isHostedTransportError } from "../../connection/connection";
import {
  isOfflineMarkdown,
  type DraftRemote,
  type OfflineWorkspace,
} from "../../offline/workspace";
import { FileWatchHub } from "./file-stream";
import type { NativeWatchAdapter } from "@lapis-notes/api";
import type { VaultAdapter } from "@lapis-notes/api/vault";
import type { RustVaultClient } from "./vault-client";
import { RustHostError } from "./client";

const vaultPath = (path: string) => path.replace(/^\/+|\/+$/gu, "");

type WriteOptions = Parameters<VaultAdapter["write"]>[2];

/** Browser presentation of one admitted native vault; no local file store. */
export class RustVaultAdapter implements VaultAdapter, NativeWatchAdapter {
  private readonly fileWatches: FileWatchHub;
  constructor(
    readonly client: RustVaultClient,
    readonly name: string,
    private readonly offline?: OfflineWorkspace,
  ) {
    this.fileWatches = new FileWatchHub(client);
  }

  getVaultId(): string {
    return this.client.descriptor.vault_id;
  }
  getName(): string {
    return this.name;
  }
  getCapabilities() {
    return {
      persistent: true,
      userVisibleFiles: false,
      requiresPermission: false,
      nativeWatch: true,
      resourceUrls: true,
      systemTrash: false,
      exclusiveCreate: true,
      conditionalTextWrite: true,
    } as const;
  }

  watch: NativeWatchAdapter["watch"] = (path, options, listener) =>
    this.fileWatches.watch(path, options, listener);

  async stat(path: string): ReturnType<VaultAdapter["stat"]> {
    const value = await this.client.files({
      operation: "stat",
      path: vaultPath(path),
    });
    if (value.kind !== "stat") throw new Error("Invalid hosted stat response");
    const stat = value.stat;
    if (!stat) return null;
    if (stat.type !== "file" && stat.type !== "folder")
      throw new Error("Invalid hosted file type");
    return { ...stat, type: stat.type };
  }
  async exists(path: string): Promise<boolean> {
    return (await this.stat(path)) !== null;
  }
  async read(path: string): Promise<string> {
    if (
      this.offline &&
      this.client.host.connection.phase === "offline" &&
      isOfflineMarkdown(vaultPath(path))
    ) {
      const saved = await this.#cachedNote(path);
      if (saved) return saved.draft?.text ?? saved.text;
    }
    return (await this.client.host.response(this.client.filePath(path))).text();
  }

  #cachedNote(path: string) {
    const account = this.offline?.current.grant?.account;
    return account
      ? this.offline?.getNote(account, this.getVaultId(), vaultPath(path))
      : undefined;
  }

  readonly draftRemote: DraftRemote = {
    read: async (path) => {
      try {
        const response = await this.client.host.response(
          this.client.filePath(path),
          { cache: "no-store" },
        );
        const etag = response.headers.get("etag");
        if (!etag) throw Error("Server did not supply a file revision.");
        return { etag, text: await response.text() };
      } catch (error) {
        if (error instanceof RustHostError && error.status === 404)
          return undefined;
        throw error;
      }
    },
    write: (path, text, etag) =>
      this.#upload(path, text, undefined, { "if-match": etag }),
  };
  async readBinary(path: string): Promise<ArrayBuffer> {
    return (
      await this.client.host.response(this.client.filePath(path))
    ).arrayBuffer();
  }

  async #upload(
    path: string,
    data: string | ArrayBuffer,
    options?: WriteOptions,
    condition: Record<string, string> = {},
  ) {
    const headers = new Headers(condition);
    if (options?.mtime !== undefined)
      headers.set("x-lapis-mtime", String(options.mtime));
    await this.client.host.response(this.client.filePath(path), {
      method: "PUT",
      headers,
      body: new Blob([data]),
    });
  }
  write(path: string, data: string, options?: WriteOptions): Promise<void> {
    return this.#upload(path, data, options);
  }
  writeBinary(
    path: string,
    data: ArrayBuffer,
    options?: WriteOptions,
  ): Promise<void> {
    return this.#upload(path, data, options);
  }
  writeExclusive(
    path: string,
    data: string | ArrayBuffer,
    options?: WriteOptions,
  ): Promise<void> {
    return this.#upload(path, data, options, { "if-none-match": "*" });
  }
  async writeTextIfUnchanged(
    path: string,
    data: string,
    expected: string | null,
    options?: WriteOptions,
  ): Promise<void> {
    const etag =
      expected === null
        ? undefined
        : `"${[...new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(expected)))].map((byte) => byte.toString(16).padStart(2, "0")).join("")}"`;
    const condition: Record<string, string> = etag
      ? { "if-match": etag }
      : { "if-none-match": "*" };
    const cache =
      this.offline &&
      isOfflineMarkdown(vaultPath(path)) &&
      this.offline.current.grant
        ? this.offline
        : undefined;
    const stage = async (interrupted: boolean) => {
      if (!cache) return false;
      const note = await this.#cachedNote(path);
      if (!note || (!note.draft && !interrupted)) return false;
      await cache.stage(note, data, etag);
      return true;
    };
    if (
      await stage(
        this.client.host.connection.phase === "offline" ||
          this.client.host.connection.phase === "reconnecting",
      )
    )
      return;
    try {
      await this.#upload(path, data, options, condition);
    } catch (error) {
      if (!isHostedTransportError(error) || !(await stage(true))) throw error;
    }
  }

  async process(
    path: string,
    transform: (data: string) => string,
    options?: WriteOptions,
  ): Promise<string> {
    const response = await this.client.host.response(
      this.client.filePath(path),
    );
    const etag = response.headers.get("etag");
    if (!etag) throw new Error("The server omitted the file revision");
    const next = transform(await response.text());
    if (typeof next !== "string")
      throw new TypeError("File transforms must return text");
    await this.#upload(path, next, options, { "if-match": etag });
    return next;
  }
  async append(
    path: string,
    data: string,
    options?: WriteOptions,
  ): Promise<void> {
    await this.process(path, (current) => current + data, options);
  }
  async appendBinary(
    path: string,
    data: ArrayBuffer,
    options?: WriteOptions,
  ): Promise<void> {
    const response = await this.client.host.response(
      this.client.filePath(path),
    );
    const etag = response.headers.get("etag");
    if (!etag) throw new Error("The server omitted the file revision");
    const current = new Uint8Array(await response.arrayBuffer());
    const result = new Uint8Array(current.length + data.byteLength);
    result.set(current);
    result.set(new Uint8Array(data), current.length);
    await this.#upload(path, result.buffer, options, { "if-match": etag });
  }

  async list(path: string) {
    const value = await this.client.files({
      operation: "list",
      path: vaultPath(path),
    });
    if (value.kind !== "list")
      throw new Error("Invalid hosted directory response");
    return value.listing;
  }
  async mkdir(
    path: string,
    options?: { recursive?: boolean; mode?: string },
  ): Promise<void> {
    try {
      await this.client.files({
        operation: "mkdir",
        path: vaultPath(path),
        recursive: options?.recursive ?? false,
      });
    } catch (error) {
      // VaultAdapter.mkdir is idempotent for an existing directory. Never
      // reinterpret uncertain writes or an existing file as success.
      if (
        error instanceof RustHostError &&
        error.code === "ALREADY_EXISTS" &&
        (await this.stat(path))?.type === "folder"
      )
        return;
      throw error;
    }
  }
  async rmdir(path: string, recursive: boolean): Promise<void> {
    await this.client.files({
      operation: "rmdir",
      path: vaultPath(path),
      recursive,
    });
  }
  async remove(path: string): Promise<void> {
    await this.client.files({ operation: "remove", path: vaultPath(path) });
  }
  async rename(path: string, newPath: string): Promise<void> {
    await this.client.files({
      operation: "rename",
      path: vaultPath(path),
      newPath: vaultPath(newPath),
    });
  }
  async copy(path: string, newPath: string): Promise<void> {
    await this.client.files({
      operation: "copy",
      path: vaultPath(path),
      newPath: vaultPath(newPath),
    });
  }
  getResourcePath(path: string): string {
    return this.client.fileUrl(path, true);
  }
  async getResourceUrl(path: string): Promise<string> {
    return this.getResourcePath(path);
  }
  async trashSystem(): Promise<boolean> {
    return false;
  }
  async trashLocal(path: string): Promise<void> {
    const target = `.trash/${path.replace(/^\/+|\/+$/gu, "")}`;
    await this.mkdir(target.slice(0, target.lastIndexOf("/")), {
      recursive: true,
    });
    await this.rename(path, target);
  }
}
