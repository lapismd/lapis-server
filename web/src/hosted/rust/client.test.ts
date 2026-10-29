import { describe, expect, it, vi } from "vitest";
import { RustHttpClient } from "./client";

const admission = {
  client_id: "client",
  vault_id: "vault",
  generation: "generation",
};
const json = (value: unknown, status = 200) => Response.json(value, { status });

describe("Rust hosted transport", () => {
  it("cannot retarget byte routes through URL dot segments", async () => {
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValueOnce(json(admission))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    const client = await new RustHttpClient("", fetch).attach("vault");
    try {
      expect(() => client.fileUrl("../other-client/files/secret")).toThrow(
        "normalized",
      );
      expect(() => client.fileUrl("a/../../other-client/files/secret")).toThrow(
        "normalized",
      );
      expect(fetch).toHaveBeenCalledTimes(1);
    } finally {
      await client.close();
    }
  });
  it("uses versioned same-origin cookie requests and the current session CSRF", async () => {
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValueOnce(
        json({ protocol_major: 3, build_id: "build", browser_auth: true }),
      )
      .mockResolvedValueOnce(json({ csrf_token: "session-token" }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    const host = new RustHttpClient("", fetch);
    expect((await host.info()).protocol_major).toBe(3);
    await host.login("owner", "password");
    await host.logout();
    expect(fetch.mock.calls.map(([url]) => url)).toEqual([
      "/api/v3/system/info",
      "/api/v3/auth/login",
      "/api/v3/auth/logout",
    ]);
    for (const [, init] of fetch.mock.calls) {
      expect(init?.credentials).toBe("same-origin");
      expect(new Headers(init?.headers).get("x-lapis-protocol")).toBe("3");
    }
    expect(
      new Headers(fetch.mock.calls[2][1]?.headers).get("x-lapis-csrf"),
    ).toBe("session-token");
  });

  it("uses only admitted IDs in generated command envelopes and disposes the lease", async () => {
    const fetch = vi.fn<typeof globalThis.fetch>(async (_url, init) => {
      if (init?.method === "DELETE") return new Response(null, { status: 204 });
      const request = JSON.parse(String(init?.body));
      if (request.command)
        return json({
          protocol_version: 3,
          request_id: request.request_id,
          outcome: {
            status: "ok",
            value: {
              kind: "files",
              value: {
                kind: "list",
                listing: { files: ["Note.md"], folders: [] },
              },
            },
          },
        });
      return json(admission);
    });
    const host = new RustHttpClient("", fetch);
    const client = await host.attach("vault");
    try {
      expect(await client.files({ operation: "list", path: "" })).toEqual({
        kind: "list",
        listing: { files: ["Note.md"], folders: [] },
      });
      const request = JSON.parse(String(fetch.mock.calls[1][1]?.body));
      expect(request).toEqual({
        protocol_version: 3,
        request_id: expect.any(String),
        vault_id: "vault",
        generation: "generation",
        command: { kind: "files", operation: { operation: "list", path: "" } },
      });
      expect(client.fileUrl("/café/笔记.md", true)).toBe(
        "/api/v3/clients/client/resources/caf%C3%A9/%E7%AC%94%E8%AE%B0.md",
      );
    } finally {
      await client.close();
    }
    await client.close();
    expect(
      fetch.mock.calls.filter(([, init]) => init?.method === "DELETE"),
    ).toHaveLength(1);
    expect(() => client.requireOpen()).toThrow("closed");
  });

  it("preserves structured conditional-write failures without retry or fallback", async () => {
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValueOnce(json(admission))
      .mockResolvedValueOnce(
        json(
          {
            protocol_version: 3,
            outcome: {
              status: "error",
              error: { code: "CONFLICT", message: "File changed" },
            },
          },
          409,
        ),
      )
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    const client = await new RustHttpClient("", fetch).attach("vault");
    try {
      await expect(
        client.files({
          operation: "write-text-if-unchanged",
          path: "Note.md",
          data: "new",
          expected: "old",
          options: { mtime: null },
        }),
      ).rejects.toMatchObject({
        code: "CONFLICT",
        message: "File changed",
        status: 409,
      });
      expect(fetch).toHaveBeenCalledTimes(2);
    } finally {
      await client.close();
    }
  });

  it("rejects mismatched admission and reply identities", async () => {
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValue(json({ ...admission, vault_id: "other" }));
    await expect(new RustHttpClient("", fetch).attach("vault")).rejects.toThrow(
      "requested vault admission",
    );
    fetch
      .mockResolvedValueOnce(json(admission))
      .mockResolvedValueOnce(json({ protocol_version: 2, request_id: "stale" }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    const client = await new RustHttpClient("", fetch).attach("vault");
    try {
      await expect(
        client.files({ operation: "list", path: "" }),
      ).rejects.toThrow("unmatched command reply");
    } finally {
      await client.close();
    }
  });

  it("does not replay a write when its response is lost", async () => {
    const lost = new TypeError("connection lost");
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValueOnce(json(admission))
      .mockRejectedValueOnce(lost)
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    const client = await new RustHttpClient("", fetch).attach("vault");
    try {
      await expect(
        client.files({ operation: "remove", path: "Note.md" }),
      ).rejects.toBe(lost);
      expect(fetch).toHaveBeenCalledTimes(2);
    } finally {
      await client.close();
    }
  });
});

it("routes generated indexing controls through the admitted lease without selecting startup", async () => {
  const response = { status: null, contribution: null };
  const fetch = vi
    .fn<typeof globalThis.fetch>()
    .mockResolvedValueOnce(json(admission))
    .mockResolvedValueOnce(json(response))
    .mockResolvedValueOnce(new Response(null, { status: 204 }));
  const client = await new RustHttpClient("", fetch).attach("vault");
  expect(fetch).toHaveBeenCalledTimes(1);
  expect(await client.indexing({ kind: "status" })).toEqual(response);
  expect(fetch.mock.calls[1][0]).toBe("/api/v3/clients/client/indexing");
  expect(JSON.parse(String(fetch.mock.calls[1][1]?.body))).toEqual({
    kind: "status",
  });
  await client.close();
  await expect(client.indexing({ kind: "select_metadata" })).rejects.toThrow();
  expect(fetch).toHaveBeenCalledTimes(3);
});
