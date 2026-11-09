import { expect, it, vi } from "vitest";
import { App, MemoryAppDatabase, MemoryVaultAdapter } from "@lapis-notes/api";
import { render } from "@testing-library/svelte";
import PluginTestRoot from "./PluginTestRoot.svelte";
import { getWorkspaceHostBinding } from "@lapis-notes/api/workspace-host";
import { contentFixture } from "../../../stories/fixtures/content-access";
import { contentAccessPlugin } from "./plugin";

it("clears the renderer's credential before plugin cleanup unregisters its source", async () => {
  const adapter = new MemoryVaultAdapter();
  const database = new MemoryAppDatabase(adapter.getVaultId());
  let app!: App;
  const view = render(PluginTestRoot, {
    create: () => {
      app = new App({
        version: "1.0.0",
        adapter,
        appDatabase: database,
        markdownRenderer: async () => {},
      });
    },
  });
  const host = contentFixture(() => [
    { id: "notes", name: "Notes", role: "owner" },
  ]);
  const Plugin = contentAccessPlugin(host, "notes");
  const plugin = new Plugin(app);
  plugin.configureRuntime({ source: "system" });
  const settings = getWorkspaceHostBinding(app.workspace).controller.settings;
  try {
    plugin.load();
    await vi.waitFor(() =>
      expect(settings.get("content-access.status")).toBe("Disabled"),
    );
    expect(await settings.set("content-access.enabled", true)).toBe(true);
    expect(await settings.runAction("content-access.create")).toBe(true);
    expect(settings.get("content-access.token")).toMatch(/^lc1_/);
    plugin.unload();
    expect(settings.get("content-access.token")).toBe("");
    expect(settings.getSnapshot().values).not.toHaveProperty(
      "content-access.token",
    );
  } finally {
    plugin.unload();
    await app.workspace.disposeWorkspaceHost();
    await database.close();
    await view.unmount();
  }
});
