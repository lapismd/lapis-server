import type { TerminalSessionClient } from "@lapis-notes/lapis-plugin-terminal";
import type {
  TerminalCommand,
  TerminalValue,
} from "../../../../crates/lapis-app-protocol/bindings/protocol";
import type { RustVaultClient } from "./vault-client";
import { readTerminalStream } from "./terminal-stream";
import { RustHostError } from "./client";

const UPDATE_SERVER =
  "Update the server to use its authenticated Terminal API.";

export function createHostedTerminal(
  client: RustVaultClient,
  allowed: () => boolean,
  report: (error: unknown) => void,
  changed: () => void = () => {},
) {
  const outputs = new Set<Parameters<TerminalSessionClient["onOutput"]>[0]>();
  const exits = new Set<Parameters<TerminalSessionClient["onExit"]>[0]>();
  const streams = new Map<string, AbortController>();
  const owned = new Set<string>();
  let epoch = 0;
  let closed = false;
  let unsupported = false;
  let retiring = 0;
  const actions = new Map<string, Promise<void>>();
  const pending = new Set<Promise<unknown>>();
  const creations = new Set<Promise<unknown>>();
  const available = () =>
    !closed &&
    !unsupported &&
    retiring === 0 &&
    !client.signal.aborted &&
    allowed();
  const call = (command: TerminalCommand): Promise<TerminalValue> => {
    client.requireOpen();
    return client.host.json(`${client.path}/terminal`, "POST", command);
  };
  function missingRoute(error: unknown): boolean {
    if (
      !(error instanceof RustHostError) ||
      error.status !== 404 ||
      error.code !== "IO"
    )
      return false;
    unsupported = true;
    changed();
    return true;
  }
  function enqueue(
    id: string,
    action: () => Promise<void>,
    cleanup = false,
  ): Promise<void> {
    const previous = actions.get(id) ?? Promise.resolve();
    const task = (cleanup ? previous.catch(() => {}) : previous).then(action);
    actions.set(id, task);
    void task.then(
      () => {
        if (actions.get(id) === task) actions.delete(id);
      },
      () => {},
    );
    return task;
  }
  async function closeAll() {
    retiring++;
    ++epoch;
    for (const controller of streams.values()) controller.abort();
    streams.clear();
    owned.clear();
    try {
      await Promise.allSettled([...creations, ...actions.values()]);
      actions.clear();
      if (!client.signal.aborted && !unsupported) {
        try {
          await call({ kind: "close_all" });
        } catch (error) {
          if (!missingRoute(error)) throw error;
        }
      }
      await Promise.allSettled([...pending]);
    } finally {
      retiring--;
    }
  }
  async function create(
    options: Parameters<TerminalSessionClient["create"]>[0] = {},
  ) {
    if (!available()) throw Error("Terminal requires a trusted vault");
    if (options.shell || options.workspace)
      throw Error("Hosted terminals use the admitted vault and host shell");
    const generation = epoch;
    let value: TerminalValue;
    try {
      value = await call({
        kind: "create",
        cwd: options.cwd ?? null,
        cols: options.cols ?? 80,
        rows: options.rows ?? 24,
      });
    } catch (error) {
      // An unstructured 404 identifies an older executable with no route. A
      // structured native NOT_FOUND remains a real vault/session error.
      if (missingRoute(error)) throw Error(UPDATE_SERVER);
      throw error;
    }
    if (value.kind !== "created")
      throw Error("Invalid terminal creation reply");
    const id = value.session_id;
    if (generation !== epoch || !available()) {
      await call({ kind: "stop", session_id: id });
      throw Error("Terminal admission changed");
    }
    owned.add(id);
    const controller = new AbortController();
    streams.set(id, controller);
    const run = (async () => {
      let cursor = 0;
      let finished = false;
      while (!controller.signal.aborted && !finished) {
        const response = await client.host.response(
          `${client.path}/terminal/${encodeURIComponent(id)}/events?cursor=${cursor}`,
          { signal: controller.signal },
        );
        await readTerminalStream(
          response,
          controller.signal,
          (output) => {
            if (!available() || generation !== epoch) return;
            cursor = output.cursor;
            if (output.truncated)
              report(Error("Terminal replay was truncated"));
            if (output.bytes.length)
              for (const listener of outputs)
                listener(id, new Uint8Array(output.bytes));
            if (!output.running && output.bytes.length === 0) {
              finished = true;
              for (const listener of exits) listener(id, output.exit_code);
            }
          },
          cursor,
        );
        // Yield the HTTP/1 connection to file/editor commands and other tabs.
        if (!finished && !controller.signal.aborted)
          await yieldConnection(controller.signal);
      }
    })()
      .catch((error) => {
        if (!controller.signal.aborted) {
          report(error);
          for (const listener of exits) listener(id, null);
        }
      })
      .finally(() => {
        streams.delete(id);
        pending.delete(run);
      });
    pending.add(run);
    return { sessionId: id, status: "running" };
  }
  const sessions: TerminalSessionClient = {
    get available() {
      return available();
    },
    get reason() {
      if (unsupported) return UPDATE_SERVER;
      return available()
        ? null
        : "Trust this vault and enable optional plugins to use Terminal.";
    },
    create(options = {}) {
      const creating = create(options);
      creations.add(creating);
      void creating.then(
        () => creations.delete(creating),
        () => creations.delete(creating),
      );
      return creating;
    },
    write(id, data) {
      return enqueue(id, async () => {
        if (!available() || !owned.has(id))
          throw Error("Terminal admission changed");
        await call({ kind: "write", session_id: id, data });
      });
    },
    resize(id, cols, rows) {
      return enqueue(id, async () => {
        if (!available() || !owned.has(id))
          throw Error("Terminal admission changed");
        await call({ kind: "resize", session_id: id, cols, rows });
      });
    },
    stop(id) {
      return enqueue(
        id,
        async () => {
          if (!owned.has(id)) return;
          streams.get(id)?.abort();
          streams.delete(id);
          await call({ kind: "stop", session_id: id });
          owned.delete(id);
        },
        true,
      );
    },
    onOutput(listener) {
      outputs.add(listener);
      return () => outputs.delete(listener);
    },
    onExit(listener) {
      exits.add(listener);
      return () => exits.delete(listener);
    },
  };
  const abort = () => {
    closed = true;
    for (const controller of streams.values()) controller.abort();
  };
  client.signal.addEventListener("abort", abort, { once: true });
  return {
    sessions,
    closeAll,
    async dispose() {
      closed = true;
      client.signal.removeEventListener("abort", abort);
      await closeAll();
      outputs.clear();
      exits.clear();
    },
  };
}

function yieldConnection(signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    const finish = () => {
      clearTimeout(timer);
      signal.removeEventListener("abort", finish);
      resolve();
    };
    const timer = setTimeout(finish, 50);
    signal.addEventListener("abort", finish, { once: true });
    if (signal.aborted) finish();
  });
}
