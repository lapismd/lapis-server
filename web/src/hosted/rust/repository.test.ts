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
it("exposes both remote targets through the same admitted Repository factory", async () => {
  const { resource, fetch } = await fixture();
  const service = resource.repositoryFactory!({} as App);
  expect(service.remotes).toBeDefined();
  const snapshot = {
    revision: 9,
    capabilities: { targets: ["backup", "original"] },
    settings: {
      host: "server",
      storage: "locked",
      targets: [],
      automaticSync: false,
    },
  };
  fetch.mockResolvedValueOnce(Response.json(snapshot));
  expect(await service.remotes!.read()).toEqual(snapshot);
  const command = {
    kind: "fetch",
    target: "original",
    expectedRevision: 9,
  } as const;
  fetch.mockResolvedValueOnce(Response.json(snapshot));
  await service.remotes!.execute(command);
  expect(
    fetch.mock.calls
      .slice(1)
      .map(([url, request]) => [url, request?.method, request?.body]),
  ).toEqual([
    [
      "/api/v3/clients/client/repository-remotes?vault=vault&generation=gen",
      "GET",
      undefined,
    ],
    [
      "/api/v3/clients/client/repository-remotes?vault=vault&generation=gen",
      "POST",
      JSON.stringify(command),
    ],
  ]);
  await service.close("close");
  await expect(service.remotes!.read()).rejects.toThrow("closed");
  expect(fetch).toHaveBeenCalledTimes(3);
});

it("exposes native synchronization through the production Repository and fences close", async () => {
  const { resource, fetch } = await fixture();
  const service = resource.repositoryFactory!({} as App);
  expect(service.remotes!.synchronization).toBeDefined();
  const synchronization = service.remotes!.synchronization!;
  const snapshot = {
    revision: 19,
    enabled: true,
    busy: false,
    phase: "attention",
    recovery: true,
    pendingApply: "retained-apply",
  };
  fetch.mockResolvedValueOnce(Response.json(snapshot));
  expect(await synchronization.read()).toEqual(snapshot);
  const command = {
    kind: "resume_apply",
    expected_revision: 19,
    request_id: "retained-apply",
  } as const;
  fetch.mockRejectedValueOnce(new Error("Lost response"));
  await expect(synchronization.execute(command)).rejects.toThrow(
    "Lost response",
  );
  const path =
    "/api/v3/clients/client/repository-synchronization?vault=vault&generation=gen";
  expect(
    fetch.mock.calls
      .slice(1)
      .map(([url, request]) => [url, request?.method, request?.body]),
  ).toEqual([
    [path, "GET", undefined],
    [path, "POST", JSON.stringify(command)],
  ]);
  let deliver!: (response: Response) => void;
  fetch.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        deliver = resolve;
      }),
  );
  const pending = synchronization.read();
  const refused = expect(pending).rejects.toThrow("closed");
  await vi.waitFor(() => expect(deliver).toBeTypeOf("function"));
  let drained = false;
  const closing = service.close("close").then(() => {
    drained = true;
  });
  await Promise.resolve();
  expect(drained).toBe(false);
  await expect(synchronization.execute(command)).rejects.toThrow("closed");
  deliver(Response.json(snapshot));
  await refused;
  await closing;
  expect(drained).toBe(true);
  expect(fetch.mock.calls.filter(([url]) => String(url) === path)).toHaveLength(
    3,
  );
});

it("parses native working-copy status without opening a history pin or capturing files", async () => {
  const { resource, fetch } = await fixture();
  const service = resource.repositoryFactory!({} as App);
  expect(service.workingCopy).toBeTypeOf("function");
  fetch.mockResolvedValueOnce(
    Response.json({
      version: 1,
      source: "original",
      operation: "ab".repeat(64),
      working_copy: "cd".repeat(20),
      workspace: "default",
      commit: {
        change_id: "ef".repeat(16),
        description: "Current work",
        author: {
          name: "Fixture",
          email: "fixture@example.test",
          timestamp: { millis: "1794748800000", offset_minutes: 0 },
        },
      },
      files: 3,
      lines: { additions: 7, deletions: 2 },
      uncounted: 0,
    }),
  );
  expect(await service.workingCopy!()).toMatchObject({
    source: "original",
    files: 3,
    lines: { additions: 7, deletions: 2 },
    author: { timestamp: { millis: 1794748800000n } },
  });
  expect(fetch.mock.calls[1][0]).toBe(
    "/api/v3/clients/client/repository-working-copy?vault=vault&generation=gen",
  );
  expect(fetch).toHaveBeenCalledTimes(2);
  await service.close("close");
  await expect(service.workingCopy!()).rejects.toThrow("closed");
  expect(fetch).toHaveBeenCalledTimes(2);
});

it("joins and fences a pending working-copy reply when the Repository observer closes", async () => {
  const { resource, fetch } = await fixture();
  const service = resource.repositoryFactory!({} as App);
  let deliver!: (response: Response) => void;
  fetch.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        deliver = resolve;
      }),
  );
  const reading = service.workingCopy!();
  const refused = expect(reading).rejects.toThrow("closed");
  await vi.waitFor(() => expect(deliver).toBeTypeOf("function"));
  let drained = false;
  const closing = service.close("close").then(() => {
    drained = true;
  });
  await Promise.resolve();
  expect(drained).toBe(false);
  await expect(service.workingCopy!()).rejects.toThrow("closed");
  deliver(
    Response.json({
      version: 1,
      source: "original",
      operation: "ab".repeat(64),
      working_copy: "cd".repeat(20),
      workspace: "default",
      commit: {
        change_id: "ef".repeat(16),
        description: "Current work",
        author: {
          name: "Fixture",
          email: "fixture@example.test",
          timestamp: { millis: "1794748800000", offset_minutes: 0 },
        },
      },
      files: 0,
      lines: { additions: 0, deletions: 0 },
      uncounted: 0,
    }),
  );
  await refused;
  await closing;
  expect(drained).toBe(true);
  expect(fetch).toHaveBeenCalledTimes(2);
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
