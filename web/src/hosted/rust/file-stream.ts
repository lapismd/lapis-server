import type {
  ChangeEvent,
  WatchErrorEvent,
  WatchOptions,
  NativeWatchSubscription,
} from "@lapis-notes/api";
import type { FileChangeSet } from "../../../../crates/lapis-app-protocol/bindings/protocol";
import type { RustVaultClient } from "./vault-client";
import { RustHostError } from "./client";

type Frame = { event: "ready" | "change"; value: FileChangeSet };
const validPath = (p: unknown): p is string =>
  typeof p === "string" &&
  p.length > 0 &&
  p.length <= 4096 &&
  !/[\\\0]/u.test(p) &&
  p.split("/").every((s) => s && s !== "." && s !== "..");
export function decodeFileFrame(frame: string): Frame | undefined {
  const lines = frame
    .split("\n")
    .filter((line) => line && !line.startsWith(":"));
  if (!lines.length) return;
  const field = (name: string) =>
    lines
      .filter((line) => line.startsWith(name + ":"))
      .map((line) => line.slice(name.length + 1).replace(/^ /u, ""));
  const event = field("event"),
    id = field("id"),
    data = field("data");
  if (
    event.length !== 1 ||
    !["ready", "change"].includes(event[0]) ||
    id.length !== 1 ||
    !data.length
  )
    throw Error("Invalid native file stream frame");
  const value = JSON.parse(data.join("\n"));
  if (
    !value ||
    !Number.isSafeInteger(value.revision) ||
    value.revision < 0 ||
    id[0] !== String(value.revision) ||
    typeof value.reset !== "boolean" ||
    !Array.isArray(value.changes) ||
    value.changes.length > 64 ||
    !value.changes.every(
      (c: { type?: string; path?: unknown; previous_path?: unknown }) =>
        c &&
        validPath(c.path) &&
        ["create", "modify", "delete", "rename"].includes(c.type || "") &&
        (c.type !== "rename" || validPath(c.previous_path)),
    )
  )
    throw Error("Invalid native file change");
  return { event: event[0] as Frame["event"], value };
}
export async function readFileStream(
  response: Response,
  accept: (frame: Frame) => void,
  signal: AbortSignal,
): Promise<void> {
  if (
    !response.headers.get("content-type")?.startsWith("text/event-stream") ||
    !response.body
  )
    throw Error("Native file stream is unavailable");
  const reader = response.body.getReader();
  const abort = () => {
    void reader.cancel().catch(() => {});
  };
  signal.addEventListener("abort", abort, { once: true });
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let buffer = "";
  try {
    while (!signal.aborted) {
      const { value, done } = await reader.read();
      if (signal.aborted) return;
      try {
        buffer += decoder.decode(value, { stream: !done });
      } catch {
        throw Error("Invalid UTF-8 in native file stream");
      }
      buffer = buffer.replaceAll("\r\n", "\n");
      let end;
      while ((end = buffer.indexOf("\n\n")) !== -1) {
        if (
          new TextEncoder().encode(buffer.slice(0, end)).byteLength >
          128 * 1024
        )
          throw Error("Native file stream exceeds frame limit");
        const frame = decodeFileFrame(buffer.slice(0, end));
        buffer = buffer.slice(end + 2);
        if (frame) accept(frame);
      }
      if (new TextEncoder().encode(buffer).byteLength > 128 * 1024)
        throw Error("Native file stream exceeds frame limit");
      if (done) {
        if (buffer.trim()) throw Error("Truncated native file stream");
        return;
      }
    }
  } finally {
    signal.removeEventListener("abort", abort);
    await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
}
type Listener = (event: ChangeEvent | WatchErrorEvent) => void;
type Slot = { paths: string[]; recursive: boolean; accept: Listener };
const normalize = (p: string) => {
  const value = p.replace(/^\/+|\/+$/gu, "");
  if (value && !validPath(value)) throw Error("Invalid native watch path");
  return value;
};
const matches = (slot: Slot, path: string) =>
  slot.paths.some(
    (base) =>
      path === base ||
      (slot.recursive && (!base || path.startsWith(base + "/"))) ||
      (!slot.recursive &&
        (base
          ? path.startsWith(base + "/") &&
            !path.slice(base.length + 1).includes("/")
          : !path.includes("/"))),
  );
function pause(signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    const finish = () => {
      clearTimeout(timer);
      signal.removeEventListener("abort", finish);
      resolve();
    };
    const timer = setTimeout(finish, 500);
    signal.addEventListener("abort", finish, { once: true });
    if (signal.aborted) finish();
  });
}
/** One stream per admitted adapter, shared by content and targeted config watches. */
export class FileWatchHub {
  #slots = new Set<Slot>();
  #controller?: AbortController;
  #done?: Promise<void>;
  #ready?: Promise<void>;
  #cursor?: number;
  constructor(private readonly client: RustVaultClient) {}
  watch(
    path: string | string[],
    options: WatchOptions,
    accept: Listener,
  ): NativeWatchSubscription {
    this.client.requireOpen();
    const slot = {
      paths: (Array.isArray(path) ? path : [path]).map(normalize),
      recursive: options.recursive ?? false,
      accept,
    };
    this.#slots.add(slot);
    if (!this.#controller) this.#start();
    return {
      ready: this.#ready,
      close: async () => {
        if (!this.#slots.delete(slot)) return;
        if (!this.#slots.size) {
          const done = this.#done;
          const controller = this.#controller;
          this.#controller = undefined;
          controller?.abort();
          await done;
        }
      },
    };
  }
  #report(error: unknown) {
    for (const slot of [...this.#slots])
      slot.accept({ type: "error", path: slot.paths[0] || "/", error });
  }
  #deliver(frame: Frame) {
    const { value } = frame;
    if (
      this.#cursor !== undefined &&
      value.revision < this.#cursor &&
      !value.reset
    )
      throw Error("Native file stream revision moved backwards");
    this.#cursor = value.revision;
    if (value.reset)
      this.#report(Error("Native file observation requires reconciliation"));
    for (const change of value.changes) {
      for (const slot of [...this.#slots]) {
        if (change.type === "rename") {
          const before = matches(slot, change.previous_path),
            after = matches(slot, change.path);
          if (before && after)
            slot.accept({
              type: "modify",
              path: change.path,
              previousPath: change.previous_path,
            });
          else if (before)
            slot.accept({ type: "delete", path: change.previous_path });
          else if (after) slot.accept({ type: "create", path: change.path });
        } else if (matches(slot, change.path)) slot.accept(change);
      }
    }
  }
  #start() {
    const controller = new AbortController();
    this.#controller = controller;
    const abort = () => controller.abort();
    this.client.signal.addEventListener("abort", abort, { once: true });
    if (this.client.signal.aborted) abort();
    let ready!: () => void, reject!: (error: unknown) => void;
    this.#ready = new Promise<void>((resolve, fail) => {
      ready = resolve;
      reject = fail;
    });
    const registration = setTimeout(() => {
      reject(Error("Native file registration timed out"));
      controller.abort();
    }, 15_000);
    void this.#ready.finally(() => clearTimeout(registration)).catch(() => {});
    this.#done = (async () => {
      let registered = false;
      while (!controller.signal.aborted) {
        try {
          this.client.requireOpen();
          const scope = new URLSearchParams({
            vault_id: this.client.descriptor.vault_id,
            generation: this.client.descriptor.generation,
          });
          if (this.#cursor !== undefined)
            scope.set("cursor", String(this.#cursor));
          const response = await this.client.host.response(
            `${this.client.path}/file-events?${scope}`,
            {
              signal: AbortSignal.any([
                controller.signal,
                AbortSignal.timeout(45_000),
              ]),
              headers: { accept: "text/event-stream" },
            },
          );
          let welcomed = false;
          await readFileStream(
            response,
            (frame) => {
              this.client.requireOpen();
              if (!welcomed && frame.event !== "ready")
                throw Error("Native file stream omitted registration");
              if (welcomed && frame.event === "ready")
                throw Error("Native file stream duplicated registration");
              this.#deliver(frame);
              if (!welcomed) {
                welcomed = true;
                registered = true;
                ready();
              }
            },
            controller.signal,
          );
          if (!welcomed && !controller.signal.aborted)
            throw Error("Native file registration was incomplete");
        } catch (error) {
          if (controller.signal.aborted) break;
          if (
            !(error instanceof TypeError) &&
            !(
              (error instanceof Error || error instanceof DOMException) &&
              ["AbortError", "TimeoutError"].includes(error.name)
            ) &&
            !(
              error instanceof RustHostError &&
              [429, 500, 502, 503, 504].includes(error.status)
            )
          ) {
            reject(error);
            this.#report(error);
            return;
          }
          // Retained replay repairs transient reconnects without a tree scan.
        }
        if (!controller.signal.aborted) await pause(controller.signal);
      }
      if (!registered) reject(Error("Native file registration was retired"));
    })().finally(() => {
      this.client.signal.removeEventListener("abort", abort);
      if (this.#controller === controller) this.#controller = undefined;
    });
  }
}
