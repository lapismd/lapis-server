import { afterEach, expect, it, vi } from "vitest";
import { createRustPluginAssetServer } from "./plugin-assets";
import { RustHttpClient } from "./client";
import { RustVaultClient } from "./vault-client";

const clients: RustVaultClient[] = [];
afterEach(async () => {
  await Promise.all(clients.splice(0).map((client) => client.close()));
});
const hash = "4bf5122f344554c53bde2ebb8cd2b7e3d1600ad631c385a5d7cce23c7785459a";
function fixture() {
  const host = new RustHttpClient(
    "https://notes.test",
    vi.fn(async () => new Response(null, { status: 204 })),
  );
  const client = RustVaultClient.admitted(
    host,
    { vault_id: "vault", client_id: "admission", generation: "generation" },
    "vault",
  );
  clients.push(client);
  const record = {
    pluginId: "portable",
    installedVersion: "1.0.0",
    files: ["z.js", "a.js", "A.js", "Z.js", "\ue000.js", "😀.js"].map(
      (path) => ({ path, size: 1, sha256: hash }),
    ),
  };
  const read = vi.fn(async () =>
    JSON.stringify({ schemaVersion: 1, plugins: { portable: record } }),
  );
  return {
    client,
    record,
    read,
    assets: createRustPluginAssetServer(client, { read }),
  };
}
const request = {
  pluginId: "portable",
  pluginPath: "/not-a-native-authority",
  relativePath: "A.js",
  version: "1.0.0",
};

it("uses the current admission and complete public UTF16 installation fingerprint", async () => {
  const f = fixture();
  expect(await f.assets.getPluginAssetUrl(request)).toBe(
    "https://notes.test/__lapis/plugins/admission/portable/1.0.0/19f31e9afb4637cb065a41d52283152f5cd4e0e8cf606a83dee7e9383766e22e/A.js",
  );
  expect(f.read).toHaveBeenCalledWith(".lapis/installed-plugins.json");
  expect(
    await f.assets.getPluginAssetUrl({ ...request, relativePath: "😀.js" }),
  ).toMatch(/\/%F0%9F%98%80.js$/u);
});
it("rejects absent or stale installations and unsafe or unsupported paths", async () => {
  const f = fixture();
  await expect(
    f.assets.getPluginAssetUrl({ ...request, pluginId: "missing" }),
  ).rejects.toMatchObject({ code: "asset-not-installed" });
  await expect(
    f.assets.getPluginAssetUrl({ ...request, version: "2.0.0" }),
  ).rejects.toMatchObject({ code: "asset-version-mismatch" });
  await expect(
    f.assets.getPluginAssetUrl({ ...request, relativePath: "../private.js" }),
  ).rejects.toMatchObject({ code: "invalid-path" });
  await expect(
    f.assets.getPluginAssetUrl({ ...request, relativePath: "unsafe.html" }),
  ).rejects.toMatchObject({ code: "unsupported-asset-type" });
});
it("never mints asset URLs after its owning vault admission closes", async () => {
  const f = fixture();
  await f.client.close();
  await expect(f.assets.getPluginAssetUrl(request)).rejects.toThrow("closed");
  expect(f.read).not.toHaveBeenCalled();
});
