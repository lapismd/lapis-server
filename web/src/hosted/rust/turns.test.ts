import { describe, expect, it, vi } from "vitest";
import { RustHttpClient } from "./client";

const admission = {
  client_id: "client",
  vault_id: "vault",
  generation: "generation",
};
describe("Rust native turn transport", () => {
  it("uses the admitted client and detaches without cancelling accepted work", async () => {
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValueOnce(Response.json(admission))
      .mockResolvedValueOnce(Response.json({ kind: "records", records: [] }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    const client = await new RustHttpClient("", fetch).attach("vault");
    try {
      expect(await client.turns({ kind: "list" })).toEqual({
        kind: "records",
        records: [],
      });
      expect(fetch.mock.calls[1][0]).toBe("/api/v3/clients/client/turns");
      expect(fetch.mock.calls[1][1]?.method).toBe("POST");
      expect(JSON.parse(String(fetch.mock.calls[1][1]?.body))).toEqual({
        kind: "list",
      });
    } finally {
      await client.close();
    }
    await expect(client.turns({ kind: "list" })).rejects.toThrow("closed");
    expect(
      fetch.mock.calls.filter(([, init]) => init?.method === "DELETE"),
    ).toHaveLength(1);
    expect(fetch).toHaveBeenCalledTimes(3);
  });
  it("preserves uncertain submission errors without retry or a browser fallback", async () => {
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValueOnce(Response.json(admission))
      .mockRejectedValueOnce(new TypeError("connection lost"))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    const client = await new RustHttpClient("", fetch).attach("vault");
    try {
      await expect(
        client.turns({
          kind: "submit",
          input: {
            request_id: crypto.randomUUID(),
            target_id: "codex",
            scope_dir: "",
            prompt: "hello",
            context: null,
            options: {
              mode: "ask",
              model: null,
              effort: null,
              max_duration_ms: 60_000,
            },
          },
        }),
      ).rejects.toThrow("connection lost");
      expect(fetch).toHaveBeenCalledTimes(2);
    } finally {
      await client.close();
    }
  });
});
