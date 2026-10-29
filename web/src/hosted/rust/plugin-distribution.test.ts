import { afterEach, expect, it, vi } from "vitest";
import { createRustPluginDistributionFetch } from "./plugin-distribution";
import { RustHttpClient } from "./client";
import { RustVaultClient } from "./vault-client";

const clients: RustVaultClient[] = [];
afterEach(async () => {
  await Promise.all(clients.splice(0).map((client) => client.close()));
});
function fixture() {
  const fetch = vi.fn<typeof globalThis.fetch>(async (_input, init) =>
    init?.method === "DELETE"
      ? new Response(null, { status: 204 })
      : new Response("verified candidate"),
  );
  const host = new RustHttpClient("https://notes.test", fetch);
  const client = RustVaultClient.admitted(
    host,
    { vault_id: "vault", client_id: "admission", generation: "generation" },
    "vault",
  );
  clients.push(client);
  return { client, fetch, download: createRustPluginDistributionFetch(client) };
}
const source =
  "https://github.com/lapismd/lapis-plugins/releases/download/v1/wordcount.lapis-plugin";

it("routes approved release bytes through the admitted Rust host without forwarding caller credentials", async () => {
  const f = fixture();
  const controller = new AbortController();
  const response = await f.download(source, {
    headers: { authorization: "must-not-forward" },
    credentials: "include",
    signal: controller.signal,
  });
  expect(await response.text()).toBe("verified candidate");
  const [url, init] = f.fetch.mock.calls[0];
  expect(url).toBe(
    `https://notes.test/api/v3/clients/admission/plugin-bundles/download?source=${encodeURIComponent(source)}`,
  );
  expect(new Headers(init?.headers).get("x-lapis-protocol")).toBe("3");
  expect(new Headers(init?.headers).get("authorization")).toBeNull();
  expect(init?.credentials).toBe("same-origin");
  expect(init?.signal).toBe(controller.signal);
});
it("retains public catalog/media fetch and HEAD request semantics without a Node fallback", async () => {
  const f = fixture();
  const url = "https://registry.example/catalog.json";
  const options = { cache: "no-store" as const };
  await f.download(url, options);
  expect(f.fetch).toHaveBeenLastCalledWith(url, options);
  await f.download(source, { method: "HEAD" });
  expect(f.fetch.mock.calls[1][1]?.method).toBe("HEAD");
});
it("refuses new plugin downloads after the owning admission closes", async () => {
  const f = fixture();
  await f.client.close();
  const calls = f.fetch.mock.calls.length;
  await expect(async () => f.download(source)).rejects.toThrow("closed");
  expect(f.fetch.mock.calls).toHaveLength(calls);
});

it("keeps the current official registry mirror inside the same authenticated byte boundary", async () => {
  const f = fixture();
  const mirror = "https://registry.lapis.md/download/wordcount/0.1.2";
  await f.download(mirror);
  expect(f.fetch.mock.calls[0][0]).toBe(
    `https://notes.test/api/v3/clients/admission/plugin-bundles/download?source=${encodeURIComponent(mirror)}`,
  );
});
