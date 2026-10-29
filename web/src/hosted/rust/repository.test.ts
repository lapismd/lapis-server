import { afterEach, expect, it, vi } from "vitest";
import type { App } from "@lapis-notes/api";
import type { HostedResources } from "../backend";
import { createRustHostedBackend } from "./backend";
import { RustHttpClient } from "./client";

const resources: HostedResources[] = [];
afterEach(async () => {
  for (const r of resources.splice(0)) await r.release();
});
const rawHeads = `{"version":1,"data":{"version":1,"profile":"git_sha1_jj045","repository":"${"cd".repeat(16)}","generation":18446744073709551615,"heads":[]}}`;
async function fixture(selected = true) {
  const fetch = vi.fn<typeof globalThis.fetch>(async (url, request) => {
    if (request?.method === "DELETE")
      return new Response(null, { status: 204 });
    if (String(url).endsWith("/clients"))
      return Response.json({
        client_id: "client",
        vault_id: "vault",
        generation: "gen",
      });
    return new Response(rawHeads);
  });
  const backend = createRustHostedBackend(new RustHttpClient("", fetch), {
    nativeHistory: selected,
  });
  const resource = await backend.prepare({
    id: "vault",
    name: "Notes",
    kind: "server-hosted",
    role: "owner",
    archived: false,
    createdAt: 0,
    updatedAt: 0,
  });
  resources.push(resource);
  return { resource, fetch };
}
it("keeps ordinary hosted Repository unselected", async () => {
  const { resource, fetch } = await fixture(false);
  expect(resource.repositoryFactory).toBeUndefined();
  expect(fetch).toHaveBeenCalledTimes(1);
});
it("binds the retained Repository factory to the same admission with lossless native integers", async () => {
  const { resource, fetch } = await fixture();
  expect(resource.repositoryFactory).toBeTypeOf("function");
  const service = resource.repositoryFactory!({} as App);
  const heads = await service.reader.heads();
  expect(heads.generation).toBe(18446744073709551615n);
  expect(fetch.mock.calls[1][0]).toBe("/api/v3/clients/client/history/heads");
  expect(fetch.mock.calls[1][1]?.body).toBe(
    '{"version":1,"data":{"vault":"vault"}}',
  );
  await service.close("close");
});
it("preserves native query diagnostics instead of generic IO", async () => {
  const { resource, fetch } = await fixture();
  const service = resource.repositoryFactory!({} as App);
  const native = {
    invalid_query: {
      kind: "invalid_expression",
      message: "Expected closing parenthesis",
    },
  };
  fetch.mockResolvedValueOnce(
    Response.json(
      {
        code: "INVALID_REQUEST",
        message: "Expected closing parenthesis",
        history: native,
      },
      { status: 400 },
    ),
  );
  await expect(service.reader.heads()).rejects.toEqual(native);
  await service.close("close");
});
it("fences accepted replies and later reads after admitted client release", async () => {
  const { resource, fetch } = await fixture();
  const service = resource.repositoryFactory!({} as App);
  let deliver!: (value: Response) => void;
  fetch.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        deliver = resolve;
      }),
  );
  const read = service.reader.heads();
  const failure = expect(read).rejects.toThrow("closed");
  await vi.waitFor(() => expect(deliver).toBeTypeOf("function"));
  await resource.release();
  deliver(new Response(rawHeads));
  await failure;
  await expect(service.reader.heads()).rejects.toThrow("closed");
  await service.close("close");
});

it("binds retained restore to the same admitted client and keeps exact request identities", async () => {
  const { resource, fetch } = await fixture();
  const service = resource.repositoryFactory!({} as App);
  const request = {
    request_id: "stable",
    checkpoint: "checkpoint",
    path: "Note.md",
    expected: "exact",
  };
  fetch.mockResolvedValueOnce(
    Response.json({
      checkpoint: "checkpoint",
      path: "Note.md",
      expected: "exact",
      bytes: 12,
      deleted: false,
    }),
  );
  expect(await service.restore!.preview("checkpoint", "Note.md")).toMatchObject(
    { expected: "exact" },
  );
  fetch.mockResolvedValueOnce(Response.json({ changed: false }));
  await service.restore!.apply(request);
  expect(
    fetch.mock.calls
      .slice(1)
      .map(([url, init]) => [url, JSON.parse(String(init!.body))]),
  ).toEqual([
    [
      "/api/v3/clients/client/repository-restore",
      { kind: "preview", checkpoint: "checkpoint", path: "Note.md" },
    ],
    ["/api/v3/clients/client/repository-restore", { kind: "apply", request }],
  ]);
  await service.close("close");
});

it("fences a restore acknowledgement after admitted client release without replay", async () => {
  const { resource, fetch } = await fixture();
  const service = resource.repositoryFactory!({} as App);
  let deliver!: (value: Response) => void;
  fetch.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        deliver = resolve;
      }),
  );
  const request = {
    request_id: "pending",
    checkpoint: "checkpoint",
    path: "Note.md",
    expected: "exact",
  };
  const apply = service.restore!.apply(request);
  const failed = expect(apply).rejects.toThrow("closed");
  await vi.waitFor(() => expect(deliver).toBeTypeOf("function"));
  await resource.release();
  deliver(Response.json({ changed: false }));
  await failed;
  await expect(service.restore!.apply(request)).rejects.toThrow("closed");
  expect(
    fetch.mock.calls.filter(([url]) =>
      String(url).endsWith("/repository-restore"),
    ),
  ).toHaveLength(1);
  await service.close("close");
});
