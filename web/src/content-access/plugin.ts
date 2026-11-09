import { getWorkspaceHostBinding } from "@lapis-notes/api/workspace-host";
import { Plugin, type App } from "@lapis-notes/api";
import type { RustHttpClient } from "../hosted/rust/client";
import { ContentAccessModel } from "./model";
import { definition } from "./definition";

/** The shared Notes renderer owns the screen; this plugin binds server authority. */
export function contentAccessPlugin(host: RustHttpClient, vaultId: string) {
  return class ContentAccessPlugin extends Plugin {
    #model = new ContentAccessModel(host, vaultId);
    constructor(app: App) {
      super(app, {
        id: "content-access",
        name: "Content API and MCP",
        version: "0.1.0",
        minAppVersion: "0.0.1",
        author: "Lapis",
        description: "Manage read-only access for external content clients.",
      });
    }
    onload() {
      // Plugin registrations dispose before onunload. Clear the source while
      // the settings controller is still subscribed so its cached token clears.
      this.register(() => this.#model.dispose());
      const binding = getWorkspaceHostBinding(this.app.workspace);
      this.register(
        binding.controller.configuration.optionSources.register({
          id: "content-access.vaults",
          load: () => this.#model.vaultOptions(),
        }),
      );
      this.registerSettingsSection(definition(this.#model));
      void this.#model.refresh().catch(() => {
        /* Status displays the connection failure. */
      });
    }
  };
}
