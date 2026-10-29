import { describe, expect, it, vi } from "vitest";
import { createRustHostedBackend } from "./backend";
import { RustHttpClient } from "./client";

const status = {
  kind: "status" as const,
  status: { enabled: false, configuration: null, tracked: null },
};
async function fixture(
  response: () => Promise<Response> = async () => Response.json(status),
  nativeTasks = true,
) {
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
    return response();
  });
  const host = new RustHttpClient("https://notes.test", fetch);
  await host.current();
  const resources = await createRustHostedBackend(host, {
    nativeTasks,
  }).prepare({
    id: "vault",
    name: "Notes",
    kind: "server-hosted",
    role: "owner",
    archived: false,
    createdAt: 0,
    updatedAt: 0,
  });
  return { resources, fetch };
}

describe("Rust hosted native Tasks adapter", () => {
  it("selects the required native metadata owner with staged Tasks", async () => {
    const f = await fixture();
    try {
      expect(f.resources.nativeTasks).toBeDefined();
      expect(f.resources.backgroundIndexing).toBeDefined();
    } finally {
      await f.resources.release();
    }
  });

  it("keeps native Tasks absent when the staged selection is off", async () => {
    const f = await fixture(undefined, false);
    try {
      expect(f.resources.nativeTasks).toBeUndefined();
      expect(f.resources.backgroundIndexing).toBeUndefined();
    } finally {
      await f.resources.release();
    }
  });
  it("exposes only its admitted Tasks route with current session and protocol", async () => {
    const f = await fixture();
    try {
      expect(f.resources.nativeTasks).toBeDefined();
      expect(await f.resources.nativeTasks!.call({ kind: "status" })).toEqual(
        status,
      );
      const [url, request] = f.fetch.mock.calls[2];
      expect(url).toBe("https://notes.test/api/v3/clients/client/tasks");
      expect(request?.method).toBe("POST");
      expect(request?.credentials).toBe("same-origin");
      expect(new Headers(request?.headers).get("x-lapis-protocol")).toBe("3");
      expect(new Headers(request?.headers).get("x-lapis-csrf")).toBe("csrf");
      expect(JSON.parse(String(request?.body))).toEqual({ kind: "status" });
    } finally {
      await f.resources.release();
    }
  });

  it("retains known source conflicts without retry or a renderer fallback", async () => {
    const failure = {
      name: "TaskMutationError",
      code: "stale-task",
      message: "Task changed on disk",
      parts: ["task"],
      request_id: "original-request",
      uncertain: false,
      operation: null,
    };
    const f = await fixture(async () =>
      Response.json({ kind: "failure", failure }),
    );
    try {
      expect(f.resources.nativeTasks).toBeDefined();
      expect(
        await f.resources.nativeTasks!.call({
          kind: "retry",
          request_id: "original-request",
        }),
      ).toEqual({ kind: "failure", failure });
      expect(f.fetch).toHaveBeenCalledTimes(3);
    } finally {
      await f.resources.release();
    }
  });

  it("never replays a lost response or uses a closed admission", async () => {
    const lost = new TypeError("Response lost");
    const f = await fixture(async () => {
      throw lost;
    });
    try {
      expect(f.resources.nativeTasks).toBeDefined();
      await expect(
        f.resources.nativeTasks!.call({
          kind: "retry",
          request_id: "original-request",
        }),
      ).rejects.toBe(lost);
      expect(f.fetch).toHaveBeenCalledTimes(3);
      await f.resources.release();
      await expect(
        f.resources.nativeTasks!.call({ kind: "status" }),
      ).rejects.toThrow("closed");
      expect(f.fetch).toHaveBeenCalledTimes(4);
    } finally {
      await f.resources.release();
    }
  });

  it("retains an accepted response after passive release without disabling native work", async () => {
    let finish!: (value: Response) => void;
    const f = await fixture(
      () => new Promise<Response>((resolve) => (finish = resolve)),
    );
    try {
      expect(f.resources.nativeTasks).toBeDefined();
      const pending = f.resources.nativeTasks!.call({ kind: "status" });
      await vi.waitFor(() => expect(f.fetch).toHaveBeenCalledTimes(3));
      await f.resources.release();
      finish(Response.json(status));
      expect(await pending).toEqual(status);
      const requests = f.fetch.mock.calls;
      expect(requests).toHaveLength(4);
      expect(requests[3][1]?.method).toBe("DELETE");
      expect(
        requests.filter(([, request]) =>
          String(request?.body).includes('"kind":"disable"'),
        ),
      ).toHaveLength(0);
    } finally {
      await f.resources.release();
    }
  });
});
