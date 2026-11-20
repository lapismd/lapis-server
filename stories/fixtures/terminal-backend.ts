import type { App } from "@lapis-notes/api";
import type { TerminalCommand } from "../../crates/lapis-app-protocol/bindings/protocol";
import type { RustVaultClient } from "../../web/src/hosted/rust/vault-client";
import { hostedTerminalWorkspace } from "../../web/src/hosted/rust/terminal-registration";
import { createHostedFixture } from "./hosted-backend";
import { terminalTranscript, type TerminalScenario } from "./terminal-output";

export function createTerminalBackend(scenario: TerminalScenario) {
  const { backend } = createHostedFixture("vaults");
  const owner = new AbortController();
  const sessions = new Map<string, { text: string; input: string }>();
  let serial = 0;
  let app: App | undefined;
  const review = {
    getApp() {
      if (!app) throw Error("Terminal fixture is not configured");
      return app;
    },
    sessions,
  };
  const client = {
    path: "/catalog/clients/terminal",
    signal: owner.signal,
    requireOpen() {},
    host: {
      async json(_path: string, _method: string, command: TerminalCommand) {
        if (command.kind === "create") {
          const id = `catalog-${++serial}`;
          sessions.set(id, {
            text: terminalTranscript(scenario).initial,
            input: "",
          });
          return { kind: "created", session_id: id };
        }
        if (command.kind === "close_all") sessions.clear();
        else if (command.kind === "stop") sessions.delete(command.session_id);
        else if (command.kind === "write") {
          const session = sessions.get(command.session_id);
          if (!session) throw Error("Terminal session unavailable");
          const transcript =
            transcripts.get(command.session_id) ?? terminalTranscript(scenario);
          transcripts.set(command.session_id, transcript);
          for (const character of command.data) {
            if (character === "\r" || character === "\n") {
              session.text += transcript.run(session.input);
              session.input = "";
            } else if (character === "\x7f") {
              if (session.input) {
                session.input = session.input.slice(0, -1);
                session.text += "\b \b";
              }
            } else if (character >= " ") {
              session.input += character;
              session.text += character;
            }
          }
        }
        return { kind: "done" };
      },
      async response(path: string) {
        const id = path.split("/").at(-2)!;
        const cursor = Number(
          new URL(path, "http://catalog.local").searchParams.get("cursor"),
        );
        const bytes = new TextEncoder().encode(sessions.get(id)?.text ?? "");
        return new Response(
          `event: output\ndata: ${JSON.stringify({
            cursor: bytes.length,
            bytes: [...bytes.slice(cursor)],
            running: true,
            truncated: false,
            exit_code: null,
          })}\n\n`,
          { headers: { "content-type": "text/event-stream" } },
        );
      },
    },
  } as unknown as RustVaultClient;
  const transcripts = new Map<string, ReturnType<typeof terminalTranscript>>();
  const prepare = backend.prepare.bind(backend);
  backend.prepare = async (profile) => {
    const resources = await prepare(profile);
    const host = hostedTerminalWorkspace(client);
    return {
      ...resources,
      workspaceHost: {
        ...host,
        async configure(ownerApp: App) {
          app = ownerApp;
          await app.workspaceTrust.grant();
          await host.configure?.(ownerApp);
        },
      },
      async release() {
        owner.abort();
        await resources.release();
      },
    };
  };
  return Object.assign(backend, { review });
}
