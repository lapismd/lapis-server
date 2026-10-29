import { describe, expect, it, vi } from "vitest";
import { RustHttpClient } from "./client";

const admission = { client_id: "client", vault_id: "vault", generation: "gen" };
describe("Rust native queue transport", () => {
  it("uses one admitted client with protocol/CSRF and fences after detach", async () => {
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValueOnce(Response.json(admission))
      .mockResolvedValueOnce(Response.json({ kind: "queue", queue: null }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    const client = await new RustHttpClient("", fetch).attach("vault");
    try {
      expect(
        await client.queues({ kind: "read", conversation_id: "conversation" }),
      ).toEqual({ kind: "queue", queue: null });
      expect(fetch.mock.calls[1][0]).toBe("/api/v3/clients/client/queues");
      const request = fetch.mock.calls[1][1]!;
      expect(request.method).toBe("POST");
      expect(new Headers(request.headers).get("x-lapis-protocol")).toBe("3");
      expect(JSON.parse(String(request.body))).toEqual({
        kind: "read",
        conversation_id: "conversation",
      });
    } finally {
      await client.close();
    }
    await expect(
      client.queues({ kind: "read", conversation_id: "conversation" }),
    ).rejects.toThrow("closed");
    expect(fetch).toHaveBeenCalledTimes(3);
  });
  it("preserves lost acceptance without resending, then supports an explicit read", async () => {
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValueOnce(Response.json(admission))
      .mockRejectedValueOnce(new TypeError("queue reply lost"))
      .mockResolvedValueOnce(Response.json({ kind: "queue", queue: null }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    const client = await new RustHttpClient("", fetch).attach("vault");
    const input = {
      acceptance_id: crypto.randomUUID(),
      conversation_id: crypto.randomUUID(),
      after_request_id: crypto.randomUUID(),
      scope_dir: "Notes",
      target_id: "opaque",
      capture: { id: "capture", user: { id: "authored", text: "café 📝" } },
    };
    try {
      await expect(client.queues({ kind: "accept", input })).rejects.toThrow(
        "queue reply lost",
      );
      expect(fetch).toHaveBeenCalledTimes(2);
      await client.queues({
        kind: "read",
        conversation_id: input.conversation_id,
      });
      expect(
        fetch.mock.calls
          .slice(1)
          .map(([, init]) => JSON.parse(String(init?.body))),
      ).toEqual([
        { kind: "accept", input },
        { kind: "read", conversation_id: input.conversation_id },
      ]);
    } finally {
      await client.close();
    }
  });
});
