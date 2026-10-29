import { afterEach, expect, it, vi } from "vitest";
import { createRustHostedBackend } from "./backend";
import { RustHttpClient } from "./client";
import type { HostedResources } from "../backend";

const resources: HostedResources[] = [];
afterEach(async () => {
  for (const resource of resources.splice(0)) await resource.release();
});

const status = {
  passes: 1,
  indexed: 1,
  deleted: 0,
  pending: 0,
  error: null,
  active: false,
  processed: 1,
  total: 1,
  phase: "idle" as const,
};

async function fixture(nativeMetadata = false, vaultId = "vault") {
  const fetch = vi.fn<typeof globalThis.fetch>(async (url, request) => {
    if (request?.method === "DELETE")
      return new Response(null, { status: 204 });
    if (String(url).endsWith("/clients"))
      return Response.json({
        client_id: `client-${vaultId}`,
        vault_id: vaultId,
        generation: "gen",
      });
    if (String(url).endsWith("/database"))
      return Response.json({ kind: "value", value: 7 });
    return Response.json({ status, contribution: null });
  });
  const backend = createRustHostedBackend(new RustHttpClient("", fetch), {
    nativeMetadata,
  });
  const prepared = await backend.prepare({
    id: vaultId,
    name: "Notes",
    kind: "server-hosted",
    role: "owner",
    archived: false,
    createdAt: 0,
    updatedAt: 0,
  });
  resources.push(prepared);
  return { prepared, fetch };
}

it("keeps native indexing absent from the default Rust hosted profile", async () => {
  const { prepared, fetch } = await fixture();
  expect(prepared.backgroundIndexing).toBeUndefined();
  expect(fetch.mock.calls.map(([url]) => url)).toEqual([
    "/api/v3/vaults/vault/clients",
  ]);
});

it("shares the admitted client with metadata and database without eager selection", async () => {
  const { prepared, fetch } = await fixture(true);
  expect(prepared.backgroundIndexing).toBeDefined();
  expect(fetch).toHaveBeenCalledTimes(1);
  const database = await prepared.database.open({
    runtime: "rust-server",
    vaultId: "vault",
    role: "direct",
  });
  try {
    expect(await database.getChangeRevision()).toBe(7);
    expect(
      await prepared.backgroundIndexing!.call({ kind: "select_metadata" }),
    ).toEqual({ status, contribution: null });
    expect(fetch.mock.calls.map(([url]) => url)).toEqual([
      "/api/v3/vaults/vault/clients",
      "/api/v3/clients/client-vault/database",
      "/api/v3/clients/client-vault/indexing",
    ]);
  } finally {
    await database.close();
  }
});

it("forwards status and explicit rebuild through the admitted indexing route", async () => {
  const { prepared, fetch } = await fixture(true);
  await prepared.backgroundIndexing!.call({ kind: "status" });
  await prepared.backgroundIndexing!.call({ kind: "refresh", rebuild: true });
  expect(
    fetch.mock.calls.slice(1).map(([, init]) => JSON.parse(String(init?.body))),
  ).toEqual([{ kind: "status" }, { kind: "refresh", rebuild: true }]);
});

it("preserves a native failure without a second admission or fallback request", async () => {
  const { prepared, fetch } = await fixture(true);
  fetch.mockResolvedValueOnce(
    Response.json(
      { code: "IO", message: "Native metadata failed" },
      { status: 503 },
    ),
  );
  await expect(
    prepared.backgroundIndexing!.call({ kind: "refresh", rebuild: false }),
  ).rejects.toMatchObject({
    code: "IO",
    message: "Native metadata failed",
    status: 503,
  });
  expect(fetch).toHaveBeenCalledTimes(2);
  expect(fetch.mock.calls[1][0]).toBe("/api/v3/clients/client-vault/indexing");
});

it("releases only its client and fences later metadata calls", async () => {
  const { prepared, fetch } = await fixture(true);
  expect(prepared.backgroundIndexing).toBeDefined();
  await prepared.release();
  await prepared.release();
  await expect(
    prepared.backgroundIndexing!.call({ kind: "status" }),
  ).rejects.toThrow("The hosted vault client is closed");
  expect(fetch.mock.calls.map(([url, init]) => [url, init?.method])).toEqual([
    ["/api/v3/vaults/vault/clients", "POST"],
    ["/api/v3/clients/client-vault", "DELETE"],
  ]);
});

it("keeps separately prepared vault indexing bound to each vault admission", async () => {
  const first = await fixture(true, "first");
  const second = await fixture(true, "second");
  await first.prepared.backgroundIndexing!.call({ kind: "status" });
  await second.prepared.backgroundIndexing!.call({ kind: "status" });
  expect(first.fetch.mock.calls[1][0]).toBe(
    "/api/v3/clients/client-first/indexing",
  );
  expect(second.fetch.mock.calls[1][0]).toBe(
    "/api/v3/clients/client-second/indexing",
  );
});
