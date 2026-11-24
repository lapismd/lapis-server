import { describe, expect, it, vi } from "vitest";
import { createRustHostedBackend } from "./backend";
import { RustHttpClient } from "./client";

async function fixture(fail = false, selected = true) {
  const snapshot = {
    revision: 0,
    runnerId: "runner",
    vaultId: "vault",
    trusted: true,
    safeMode: false,
    nativeNotificationsAvailable: false,
    jobs: [],
    transfer: null,
  };
  const fetch = vi.fn<typeof globalThis.fetch>(async (url, request) => {
    if (String(url).endsWith("/auth/current"))
      return Response.json({ csrf_token: "csrf" });
    if (request?.method === "DELETE")
      return new Response(null, { status: 204 });
    if (String(url).endsWith("/clients"))
      return Response.json({
        client_id: "client",
        vault_id: "vault",
        generation: "generation",
      });
    if (fail) throw new TypeError("Connection lost");
    const frame = JSON.parse(String(request?.body));
    return Response.json({
      protocol_version: 3,
      request_id: frame.request_id,
      outcome: { status: "ok", value: { kind: "plugin_jobs", snapshot } },
    });
  });
  const host = new RustHttpClient("https://notes.test", fetch);
  await host.current();
  const resources = await createRustHostedBackend(host, {
    nativeMetadata: selected,
  }).prepare({
    id: "vault",
    name: "Notes",
    kind: "server-hosted",
    role: "owner",
    archived: false,
    createdAt: 0,
    updatedAt: 0,
  });
  return { fetch, resources, snapshot };
}
describe("hosted background jobs", () => {
  it("uses the authenticated admitted command transport and retires with its client", async () => {
    const f = await fixture();
    expect(
      await f.resources.nativeBackgroundJobs!.call({ kind: "list" }),
    ).toEqual(f.snapshot);
    const [url, request] = f.fetch.mock.calls[2];
    expect(url).toBe("https://notes.test/api/v3/clients/client/commands");
    expect(new Headers(request?.headers).get("x-lapis-csrf")).toBe("csrf");
    expect(JSON.parse(String(request?.body))).toMatchObject({
      vault_id: "vault",
      generation: "generation",
      command: { kind: "plugin_jobs", command: { kind: "list" } },
    });
    await f.resources.release();
    await expect(
      f.resources.nativeBackgroundJobs!.call({ kind: "list" }),
    ).rejects.toThrow();
  });
  it("never retries an uncertain manual run", async () => {
    const f = await fixture(true);
    await expect(
      f.resources.nativeBackgroundJobs!.call({
        kind: "runNow",
        pluginId: "example",
        jobId: "refresh",
      }),
    ).rejects.toThrow("Connection lost");
    expect(f.fetch).toHaveBeenCalledTimes(3);
    await f.resources.release();
  });
  it("does not admit jobs with renderer metadata", async () => {
    const f = await fixture(false, false);
    expect(f.resources.nativeBackgroundJobs).toBeUndefined();
    await f.resources.release();
  });
});
