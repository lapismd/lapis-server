import { App, applyDocumentTheme } from "@modelcontextprotocol/ext-apps";
import type { CallToolResult } from "@modelcontextprotocol/client";
import { z } from "zod";

type Callbacks = {
  input: (args: unknown) => void;
  result: (result: CallToolResult) => void;
  ready: (callsAvailable: boolean) => void;
  failed: (message: string) => void;
};

export function connectView(title: string, callbacks: Callbacks) {
  const bridge = new App({ name: title, version: "1.0.0" });
  const abort = new AbortController();
  let live = true;
  bridge.ontoolinput = ({ arguments: args }) => callbacks.input(args);
  bridge.ontoolresult = callbacks.result;
  bridge.ontoolcancelled = () =>
    callbacks.failed("The tool call was canceled.");
  bridge.onhostcontextchanged = ({ theme }) => {
    if (theme) setTheme(theme);
  };
  bridge.onteardown = () => {
    live = false;
    abort.abort();
    return {};
  };

  function setTheme(theme: "light" | "dark") {
    applyDocumentTheme(theme);
    document.documentElement.classList.toggle("dark", theme === "dark");
  }

  void bridge
    .connect(undefined, { signal: abort.signal })
    .then(() => {
      if (!live) return;
      setTheme(bridge.getHostContext()?.theme ?? "light");
      callbacks.ready(Boolean(bridge.getHostCapabilities()?.serverTools));
    })
    .catch(() => {
      if (live) callbacks.failed("The MCP host could not open this view.");
    });

  return {
    async call(name: string, args: Record<string, unknown>) {
      return bridge.callServerTool(
        { name, arguments: args },
        { signal: abort.signal, timeout: 20_000 },
      );
    },
    stop() {
      live = false;
      abort.abort();
      void bridge.close().catch(() => {});
    },
  };
}

const outcome = z.object({
  outcome: z.discriminatedUnion("status", [
    z.object({ status: z.literal("ok"), value: z.unknown() }),
    z.object({
      status: z.literal("error"),
      error: z.object({ code: z.string(), effect: z.string() }),
    }),
  ]),
});

export class ToolFailure extends Error {
  constructor(
    public readonly code: string,
    public readonly effect: string,
  ) {
    super(code);
  }
}

export function toolValue<T>(result: CallToolResult, schema: z.ZodType<T>): T {
  const reply = outcome.parse(result.structuredContent).outcome;
  if (reply.status === "error") {
    throw new ToolFailure(reply.error.code, reply.error.effect);
  }
  if (result.isError) throw new Error("Tool result was unsuccessful");
  return schema.parse(reply.value);
}

export type ToolBridge = ReturnType<typeof connectView>;
