import type { App } from "@lapis-notes/api";
import { TerminalPlugin } from "@lapis-notes/lapis-plugin-terminal";
import type { NotesWorkspaceHost } from "@lapis-notes/app-profile/workspace-session";
import { createHostedTerminal } from "./terminal-client";
import { HOSTED_TERMINAL_APPEARANCE } from "./terminal-appearance";
import type { RustVaultClient } from "./vault-client";

type StaticPluginRegistration = Parameters<
  App["plugins"]["registerStaticPlugins"]
>[0][number];

export function hostedTerminalWorkspace(
  client: RustVaultClient,
  registrations: StaticPluginRegistration[] = [],
): NotesWorkspaceHost {
  let dispose: (() => Promise<void>) | undefined;
  let configured: App | undefined;
  return {
    unavailablePlugins: {},
    async configure(app: App) {
      if (configured) {
        if (configured !== app)
          throw Error("A Terminal host cannot be retargeted to another App");
        return;
      }
      configured = app;
      let instance: TerminalPlugin | undefined;
      const terminal = createHostedTerminal(
        client,
        () =>
          app.workspaceTrust.trusted &&
          !app.safeMode.disableOptionalCorePlugins,
        (error) => {
          app.notifications.notify({
            title: "Terminal",
            source: "terminal",
            severity: "error",
            message: error instanceof Error ? error.message : String(error),
          });
        },
        () => instance?.refreshHostSessions(),
      );
      class HostedTerminalPlugin extends TerminalPlugin {
        constructor(owner: App) {
          super(
            owner,
            undefined,
            terminal.sessions,
            HOSTED_TERMINAL_APPEARANCE,
          );
          instance = this;
        }
        async onload() {
          instance = this;
          await super.onload();
        }
        async onunload() {
          try {
            await super.onunload();
          } finally {
            await terminal.closeAll();
            if (instance === this) instance = undefined;
          }
        }
      }
      app.plugins.registerStaticPlugins([
        ...registrations,
        {
          plugin: HostedTerminalPlugin,
          required: false,
          enabledByDefault: true,
        },
      ]);
      const trust = app.workspaceTrust.on("changed", () => {
        const refresh = () => instance?.refreshHostSessions();
        if (!app.workspaceTrust.trusted)
          void terminal
            .closeAll()
            .then(refresh)
            .catch((error) =>
              app.notifications.notify({
                title: "Terminal",
                message: String(error),
                severity: "error",
              }),
            );
        else refresh();
      });
      dispose = async () => {
        app.workspaceTrust.offref(trust);
        await terminal.dispose();
      };
      // Trust and PTY ownership outlive partial boot retries in this App.
      // Final host disposal releases them after plugin and workspace teardown.
    },
    async dispose() {
      await dispose?.();
    },
  };
}
