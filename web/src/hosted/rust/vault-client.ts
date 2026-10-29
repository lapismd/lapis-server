import type {
  NativeTaskCommand,
  NativeTaskValue,
} from "@lapis-notes/api/native-tasks";
import type { Command } from "@lapismd/jj-history";
import {
  PROTOCOL_MAJOR,
  type ClientDescriptor,
  type FileCommand,
  type FileValue,
  type Reply,
  type Request,
  type VaultCommand,
  type VaultValue,
  type QueueCommand,
  type QueueValue,
  type ParticipantCommand,
  type ParticipantValue,
  type TurnCommand,
  type TurnValue,
  type HistoryIndexCommand,
  type HistoryIndexValue,
  type IndexingCommand,
  type IndexingValue,
  type RepositoryRestoreCommand,
  type RepositoryRestoreValue,
  type RepositoryBackupCommand,
  type RepositoryBackupSnapshot,
  type RepositorySourcesCommand,
  type RepositorySources,
} from "../../../../crates/lapis-app-protocol/bindings/protocol";
import { RustHostError, type RustHttpClient } from "./client";

export class RustVaultClient {
  #closed = false;
  #leaseError?: unknown;
  #heartbeat: ReturnType<typeof setInterval>;
  #closePromise?: Promise<void>;
  #lifetime = new AbortController();

  private constructor(
    readonly host: RustHttpClient,
    readonly descriptor: ClientDescriptor,
  ) {
    this.#heartbeat = setInterval(() => {
      void this.host.json(`${this.path}/heartbeat`, "POST").catch((error) => {
        this.#leaseError = error;
        this.#lifetime.abort();
      });
    }, 30_000);
  }

  static admitted(host: RustHttpClient, value: unknown, vaultId: string) {
    const descriptor = value as ClientDescriptor | undefined;
    if (
      descriptor?.vault_id !== vaultId ||
      typeof descriptor.client_id !== "string" ||
      !descriptor.client_id ||
      typeof descriptor.generation !== "string" ||
      !descriptor.generation
    )
      throw new Error(
        "The server did not return the requested vault admission",
      );
    return new RustVaultClient(host, descriptor);
  }

  get path(): string {
    return `/clients/${encodeURIComponent(this.descriptor.client_id)}`;
  }
  get signal(): AbortSignal {
    return this.#lifetime.signal;
  }

  requireOpen(): void {
    if (this.#closed) throw new Error("The hosted vault client is closed");
    if (this.#leaseError) throw this.#leaseError;
  }

  filePath(path: string, resource = false): string {
    this.requireOpen();
    if (
      path.includes("\\") ||
      path.split("/").some((part) => part === "." || part === "..")
    )
      throw new Error("Vault byte paths must be normalized");
    const encoded = path
      .replace(/^\/+|\/+$/gu, "")
      .split("/")
      .map(encodeURIComponent)
      .join("/");
    return `${this.path}/${resource ? "resources" : "files"}/${encoded}`;
  }

  fileUrl(path: string, resource = false): string {
    return this.host.url(this.filePath(path, resource));
  }

  async execute(command: VaultCommand): Promise<VaultValue> {
    this.requireOpen();
    const requestId = crypto.randomUUID();
    const request: Request = {
      protocol_version: PROTOCOL_MAJOR,
      request_id: requestId,
      vault_id: this.descriptor.vault_id,
      generation: this.descriptor.generation,
      command,
    };
    const reply = await this.host.json<Reply>(
      `${this.path}/commands`,
      "POST",
      request,
    );
    if (
      reply.protocol_version !== PROTOCOL_MAJOR ||
      reply.request_id !== requestId
    )
      throw new Error("The server returned an unmatched command reply");
    if (reply.outcome.status === "error")
      throw new RustHostError(
        reply.outcome.error.code,
        reply.outcome.error.message,
        409,
      );
    return reply.outcome.value;
  }

  async files(operation: FileCommand): Promise<FileValue> {
    const value = await this.execute({ kind: "files", operation });
    if (value.kind !== "files") throw new Error("Invalid hosted file response");
    return value.value;
  }

  async turns(command: TurnCommand): Promise<TurnValue> {
    this.requireOpen();
    return this.host.json(`${this.path}/turns`, "POST", command);
  }

  async queues(command: QueueCommand): Promise<QueueValue> {
    this.requireOpen();
    const value = await this.host.json<QueueValue>(
      `${this.path}/queues`,
      "POST",
      command,
    );
    this.requireOpen();
    return value;
  }

  async participants(command: ParticipantCommand): Promise<ParticipantValue> {
    this.requireOpen();
    const value = await this.host.json<ParticipantValue>(
      `${this.path}/participants`,
      "POST",
      command,
    );
    this.requireOpen();
    return value;
  }

  async indexing(command: IndexingCommand): Promise<IndexingValue> {
    this.requireOpen();
    return this.host.json(`${this.path}/indexing`, "POST", command);
  }

  async historyIndex(command: HistoryIndexCommand): Promise<HistoryIndexValue> {
    this.requireOpen();
    return this.host.json(`${this.path}/history-index`, "POST", command);
  }

  async repositoryRestore(
    command: RepositoryRestoreCommand,
  ): Promise<RepositoryRestoreValue> {
    this.requireOpen();
    const result = await this.host.json<RepositoryRestoreValue>(
      `${this.path}/repository-restore`,
      "POST",
      command,
    );
    this.requireOpen();
    return result;
  }

  async repositoryBackup(
    command: RepositoryBackupCommand,
  ): Promise<RepositoryBackupSnapshot> {
    this.requireOpen();
    const result = await this.host.json<RepositoryBackupSnapshot>(
      `${this.path}/repository-backup`,
      "POST",
      command,
    );
    this.requireOpen();
    return result;
  }

  async repositorySources(
    command: RepositorySourcesCommand,
  ): Promise<RepositorySources> {
    this.requireOpen();
    const result = await this.host.json<RepositorySources>(
      `${this.path}/repository-sources`,
      "POST",
      command,
    );
    this.requireOpen();
    if (
      typeof result?.original !== "boolean" ||
      (result.token !== null && typeof result.token !== "string") ||
      (command.kind === "open_original" && (!result.original || !result.token))
    )
      throw new Error("Invalid repository source state");
    return result;
  }

  async history(command: Command, frame: string): Promise<string> {
    this.requireOpen();
    try {
      const response = await this.host.response(
        `${this.path}/history/${command}`,
        {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: frame,
        },
      );
      const text = await response.text();
      this.requireOpen();
      return text;
    } catch (error) {
      if (
        error instanceof RustHostError &&
        error.details &&
        typeof error.details === "object" &&
        "history" in error.details
      )
        throw error.details.history;
      throw error;
    }
  }

  async tasks(command: NativeTaskCommand): Promise<NativeTaskValue> {
    this.requireOpen();
    return this.host.json(`${this.path}/tasks`, "POST", command);
  }

  close(): Promise<void> {
    if (!this.#closePromise) {
      this.#closed = true;
      this.#lifetime.abort();
      clearInterval(this.#heartbeat);
      this.#closePromise = this.host.json(`${this.path}`, "DELETE");
    }
    return this.#closePromise;
  }
}
