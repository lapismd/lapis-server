import { expect, it, vi } from "vitest";
import type { App } from "@lapis-notes/api";
import type { RustVaultClient } from "./vault-client";
import { hostedTerminalWorkspace } from "./terminal-registration";

function fixture() {
  const json = vi.fn(async () => ({ kind: "done" }));
  const client = {
    path: "/clients/owner",
    signal: new AbortController().signal,
    requireOpen() {},
    host: { json },
  } as unknown as RustVaultClient;
  const registerStaticPlugins = vi.fn();
  const offref = vi.fn();
  const on = vi.fn(() => ({}));
  const app = {
    plugins: { registerStaticPlugins },
    workspaceTrust: { trusted: true, on, offref },
    safeMode: { disableOptionalCorePlugins: false },
    notifications: { notify: vi.fn() },
  } as unknown as App;
  return { app, client, json, registerStaticPlugins, on, offref };
}

it("registers required Content Access with optional default Terminal", async () => {
  const f = fixture();
  class ContentAccessPlugin {}
  const host = hostedTerminalWorkspace(f.client, [
    {
      plugin: ContentAccessPlugin as never,
      required: true,
      enabledByDefault: true,
    },
  ]);
  expect(host.unavailablePlugins).toEqual({});
  await host.configure!(f.app);
  expect(f.registerStaticPlugins).toHaveBeenCalledWith([
    {
      plugin: ContentAccessPlugin,
      required: true,
      enabledByDefault: true,
    },
    { plugin: expect.any(Function), required: false, enabledByDefault: true },
  ]);
  await host.dispose!(f.app);
  expect(f.json).toHaveBeenCalledWith("/clients/owner/terminal", "POST", {
    kind: "close_all",
  });
  expect(f.offref).toHaveBeenCalledOnce();
});

it("keeps both plugin registrations single through startup retries", async () => {
  const f = fixture();
  class ContentAccessPlugin {}
  const host = hostedTerminalWorkspace(f.client, [
    {
      plugin: ContentAccessPlugin as never,
      required: true,
      enabledByDefault: true,
    },
  ]);
  await host.configure!(f.app);
  await host.configure!(f.app);
  expect(f.registerStaticPlugins).toHaveBeenCalledOnce();
  expect(f.registerStaticPlugins.mock.calls[0][0]).toHaveLength(2);
  expect(f.on).toHaveBeenCalledOnce();
  await host.dispose!(f.app);
});
