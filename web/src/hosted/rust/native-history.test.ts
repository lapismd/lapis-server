import { afterEach, expect, it, vi } from "vitest";
import { createRustHostedBackend } from "./backend";
import { RustHttpClient } from "./client";
import type { HostedResources } from "../backend";

const resources: HostedResources[] = [];
afterEach(async () => {
  for (const resource of resources.splice(0)) await resource.release();
});
const status = { generation: 3, indexed: 8, omitted: 1, complete: false };
async function fixture(nativeHistory = false, vaultId = "vault") {
  const fetch = vi.fn<typeof globalThis.fetch>(async (url, request) => {
    if (request?.method === "DELETE")
      return new Response(null, { status: 204 });
    if (String(url).endsWith("/clients"))
      return Response.json({
        client_id: `client-${vaultId}`,
        vault_id: vaultId,
        generation: "gen",
      });
    return Response.json(status);
  });
  const backend = createRustHostedBackend(new RustHttpClient("", fetch), {
    nativeHistory,
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
it("leaves History absent from the ordinary hosted profile", async () => {
  const { prepared, fetch } = await fixture();
  expect(prepared.nativeHistoryIndex).toBeUndefined();
  expect(fetch).toHaveBeenCalledTimes(1);
});
it("uses the existing admission for History without eager selection", async () => {
  const { prepared, fetch } = await fixture(true);
  expect(prepared.nativeHistoryIndex).toBeDefined();
  expect(fetch).toHaveBeenCalledTimes(1);
  expect(await prepared.nativeHistoryIndex!.call({ kind: "start" })).toEqual(
    status,
  );
  expect(fetch.mock.calls[1][0]).toBe(
    "/api/v3/clients/client-vault/history-index",
  );
  expect(JSON.parse(String(fetch.mock.calls[1][1]?.body))).toEqual({
    kind: "start",
  });
});
it("forwards saved History queries and context without a batch scheduler", async () => {
  const { prepared, fetch } = await fixture(true);
  const search = {
    expression: { type: "term" as const, field: "content", value: "saved" },
    case_sensitive: true,
    cursor: "saved-cursor",
    limit: 20,
  };
  await prepared.nativeHistoryIndex!.call({ kind: "status" });
  await prepared.nativeHistoryIndex!.call({ kind: "query", search });
  await prepared.nativeHistoryIndex!.call({
    kind: "context",
    commit: "a".repeat(40),
    path: "Note.md",
    start: 5,
    end: 20,
  });
  expect(
    fetch.mock.calls
      .slice(1)
      .map(([, request]) => JSON.parse(String(request?.body))),
  ).toEqual([
    { kind: "status" },
    { kind: "query", search },
    {
      kind: "context",
      commit: "a".repeat(40),
      path: "Note.md",
      start: 5,
      end: 20,
    },
  ]);
});
it("retains native failure without a second admission or browser fallback", async () => {
  const { prepared, fetch } = await fixture(true);
  fetch.mockResolvedValueOnce(
    Response.json(
      { code: "IO", message: "Saved root replaced" },
      { status: 409 },
    ),
  );
  await expect(
    prepared.nativeHistoryIndex!.call({ kind: "status" }),
  ).rejects.toMatchObject({ code: "IO", message: "Saved root replaced" });
  expect(fetch).toHaveBeenCalledTimes(2);
});
it("detaches only its client and fences later History requests", async () => {
  const { prepared, fetch } = await fixture(true);
  expect(prepared.nativeHistoryIndex).toBeDefined();
  await prepared.release();
  await prepared.release();
  await expect(
    prepared.nativeHistoryIndex!.call({ kind: "status" }),
  ).rejects.toThrow("The hosted vault client is closed");
  expect(fetch.mock.calls.map(([url, init]) => [url, init?.method])).toEqual([
    ["/api/v3/vaults/vault/clients", "POST"],
    ["/api/v3/clients/client-vault", "DELETE"],
  ]);
});
it("keeps separate vault History ports bound to their own clients", async () => {
  const first = await fixture(true, "first"),
    second = await fixture(true, "second");
  await first.prepared.nativeHistoryIndex!.call({ kind: "status" });
  await second.prepared.nativeHistoryIndex!.call({ kind: "status" });
  expect(first.fetch.mock.calls[1][0]).toBe(
    "/api/v3/clients/client-first/history-index",
  );
  expect(second.fetch.mock.calls[1][0]).toBe(
    "/api/v3/clients/client-second/history-index",
  );
});
