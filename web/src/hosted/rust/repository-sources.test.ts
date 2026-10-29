import { afterEach, expect, it, vi } from "vitest";
import type { App } from "@lapis-notes/api";
import type { HostedResources } from "../backend";
import { createRustHostedBackend } from "./backend";
import { RustHttpClient } from "./client";

const resources: HostedResources[] = [];
afterEach(async () => {
  for (const resource of resources.splice(0)) await resource.release();
});
const heads = `{"version":1,"data":{"version":1,"profile":"git_sha1_jj045","repository":"${"cd".repeat(16)}","generation":18446744073709551615,"heads":[]}}`;
async function fixture() {
  const fetch = vi.fn<typeof globalThis.fetch>(async (url, request) => {
    if (request?.method === "DELETE")
      return new Response(null, { status: 204 });
    if (String(url).endsWith("/clients"))
      return Response.json({
        client_id: "client",
        vault_id: "vault",
        generation: "epoch",
      });
    if (String(url).endsWith("/repository-sources"))
      return Response.json({
        original: true,
        token:
          JSON.parse(String(request?.body)).kind === "open_original"
            ? "original:opaque"
            : null,
      });
    return new Response(heads);
  });
  const backend = createRustHostedBackend(new RustHttpClient("", fetch), {
    nativeHistory: true,
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
  const services = resource.repositoryFactory!({} as App);
  return { resource, fetch, services };
}

it("uses the same admission and exact native token for lazy independent original reads", async () => {
  const f = await fixture();
  expect(f.fetch).toHaveBeenCalledOnce();
  expect(await f.services.sources!()).toMatchObject({ original: true });
  expect((await f.services.original!.heads()).generation).toBe(
    18446744073709551615n,
  );
  await f.services.original!.heads();
  await f.services.reader.heads();
  expect(
    f.fetch.mock.calls
      .slice(1)
      .map(([url, init]) => [url, JSON.parse(String(init?.body))]),
  ).toEqual([
    ["/api/v3/clients/client/repository-sources", { kind: "discover" }],
    ["/api/v3/clients/client/repository-sources", { kind: "open_original" }],
    [
      "/api/v3/clients/client/history/heads",
      { version: 1, data: { vault: "original:opaque" } },
    ],
    [
      "/api/v3/clients/client/history/heads",
      { version: 1, data: { vault: "original:opaque" } },
    ],
    [
      "/api/v3/clients/client/history/heads",
      { version: 1, data: { vault: "vault" } },
    ],
  ]);
  await f.services.close("close");
});

it("fences discovery and original opening replies after client release", async () => {
  for (const command of ["discover", "open_original"]) {
    const f = await fixture();
    let deliver!: (response: Response) => void;
    f.fetch.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          deliver = resolve;
        }),
    );
    const pending =
      command === "discover"
        ? f.services.sources!()
        : f.services.original!.heads();
    const rejected = expect(pending).rejects.toThrow("closed");
    await vi.waitFor(() => expect(deliver).toBeTypeOf("function"));
    await f.resource.release();
    deliver(
      Response.json({
        original: true,
        token: command === "open_original" ? "original:opaque" : null,
      }),
    );
    await rejected;
    await expect(f.services.original!.heads()).rejects.toThrow("closed");
    expect(
      f.fetch.mock.calls.filter(([url]) => String(url).includes("/history/")),
    ).toHaveLength(0);
    await f.services.close("close");
  }
});

it("rejects malformed original sources and preserves native failures without managed fallback", async () => {
  for (const value of [
    { original: true, token: null },
    { original: false, token: "original:opaque" },
    { original: true, token: 7 },
  ]) {
    const f = await fixture();
    f.fetch.mockResolvedValueOnce(Response.json(value));
    await expect(f.services.original!.heads()).rejects.toThrow(
      "Invalid repository source",
    );
    expect(f.fetch).toHaveBeenCalledTimes(2);
    await f.services.close("close");
  }
  const f = await fixture();
  f.fetch.mockResolvedValueOnce(
    Response.json(
      { code: "IO", message: "Original source replaced" },
      { status: 409 },
    ),
  );
  await expect(f.services.original!.heads()).rejects.toMatchObject({
    code: "IO",
    message: "Original source replaced",
  });
  expect(f.fetch).toHaveBeenCalledTimes(2);
  await f.services.close("close");
});
