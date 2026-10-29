import type {
  AppDatabaseChangeSet,
  AppDatabaseChangeListener,
} from "@lapis-notes/api/app-database";
import { RustHostError } from "./client";
import type { RustVaultClient } from "./vault-client";

const MAX_FRAME = 256 * 1024;
const domains = new Set([
  "metadata",
  "search",
  "history",
  "notification",
  "notebook",
  "task",
  "projection",
  "memory",
  "meta",
]);
const strings = (value: unknown): value is string[] =>
  Array.isArray(value) &&
  value.length <= 4096 &&
  value.every((item) => typeof item === "string" && item.length <= 4096);

function decode(frame: string): AppDatabaseChangeSet | undefined {
  const fields = frame
    .split("\n")
    .filter((line) => line && !line.startsWith(":"));
  if (!fields.length) return;
  const field = (name: string) =>
    fields
      .filter((line) => line.startsWith(name + ":"))
      .map((line) => line.slice(name.length + 1).replace(/^ /u, ""));
  const id = field("id"),
    event = field("event"),
    data = field("data");
  if (
    id.length !== 1 ||
    event.length !== 1 ||
    event[0] !== "change" ||
    !data.length
  )
    throw Error("Invalid native database stream frame");
  const value = JSON.parse(data.join("\n"));
  if (
    !value ||
    !Number.isSafeInteger(value.revision) ||
    value.revision < 0 ||
    id[0] !== String(value.revision) ||
    !strings(value.domains) ||
    !value.domains.every((domain: string) => domains.has(domain)) ||
    !strings(value.paths) ||
    !Number.isSafeInteger(value.committedAt) ||
    value.committedAt < 0 ||
    (value.reset !== undefined && typeof value.reset !== "boolean") ||
    (value.renamed !== undefined &&
      (!Array.isArray(value.renamed) ||
        value.renamed.length > 4096 ||
        !value.renamed.every(
          (row: { oldPath?: unknown; newPath?: unknown }) =>
            row && strings([row.oldPath, row.newPath]),
        )))
  )
    throw Error("Invalid native database change");
  return value;
}

/** Transport reader only. It never polls a database or owns a native job. */
export async function readDatabaseStream(
  response: Response,
  accept: AppDatabaseChangeListener,
  signal: AbortSignal,
): Promise<void> {
  if (
    !response.headers.get("content-type")?.startsWith("text/event-stream") ||
    !response.body
  )
    throw Error("Native database stream is unavailable");
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
      buffer += decoder.decode(value, { stream: !done });
      buffer = buffer.replaceAll("\r\n", "\n");
      let end: number;
      while ((end = buffer.indexOf("\n\n")) !== -1) {
        if (
          new TextEncoder().encode(buffer.slice(0, end)).byteLength > MAX_FRAME
        )
          throw Error("Native database stream exceeds frame limit");
        const change = decode(buffer.slice(0, end));
        buffer = buffer.slice(end + 2);
        if (change) accept(change);
      }
      if (new TextEncoder().encode(buffer).byteLength > MAX_FRAME)
        throw Error("Native database stream exceeds frame limit");
      if (done) {
        if (buffer.trim()) throw Error("Truncated native database stream");
        return;
      }
    }
  } finally {
    signal.removeEventListener("abort", abort);
    await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
}

function delay(signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    const finish = () => {
      clearTimeout(timer);
      signal.removeEventListener("abort", finish);
      resolve();
    };
    const timer = setTimeout(finish, 1000);
    signal.addEventListener("abort", finish, { once: true });
    if (signal.aborted) finish();
  });
}

export function observeDatabaseChanges(
  client: RustVaultClient,
  accept: AppDatabaseChangeListener,
  report: (error: unknown) => void,
) {
  const controller = new AbortController();
  const abort = () => controller.abort();
  client.signal.addEventListener("abort", abort, { once: true });
  if (client.signal.aborted) abort();
  const done = (async () => {
    let revision: number | undefined;
    while (!controller.signal.aborted) {
      try {
        client.requireOpen();
        const scope = new URLSearchParams({
          vault_id: client.descriptor.vault_id,
          generation: client.descriptor.generation,
        });
        if (revision !== undefined) scope.set("cursor", String(revision));
        const response = await client.host.response(
          `${client.path}/database/events?${scope}`,
          {
            signal: controller.signal,
            headers: { accept: "text/event-stream" },
          },
        );
        await readDatabaseStream(
          response,
          (change) => {
            client.requireOpen();
            if (revision !== undefined && change.revision < revision)
              throw Error("Native database stream revision moved backwards");
            revision = change.revision;
            accept(change);
          },
          controller.signal,
        );
      } catch (error) {
        if (controller.signal.aborted) return;
        report(error);
        if (
          !(error instanceof TypeError) &&
          !(
            error instanceof RustHostError &&
            [429, 500, 502, 503, 504].includes(error.status)
          )
        )
          return;
      }
      if (!controller.signal.aborted) await delay(controller.signal);
    }
  })().finally(() => client.signal.removeEventListener("abort", abort));
  return { stop: abort, done };
}
