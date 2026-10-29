import { describe, expect, it, vi } from "vitest";
import { RustHttpClient } from "./client";
import { RustDatabaseProvider } from "./database-provider";

const admission = {
  client_id: "client",
  vault_id: "vault",
  generation: "generation",
};
const context = {
  vaultId: "vault",
  runtime: "rust-server",
  role: "direct",
} as const;
const empty = () => new Response(null, { status: 204 });

describe("Rust hosted database views", () => {
  it("requires the owning direct admission without opening another role or vault", async () => {
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValueOnce(Response.json(admission))
      .mockResolvedValueOnce(empty());
    const client = await new RustHttpClient("", fetch).attach("vault");
    try {
      const provider = new RustDatabaseProvider(client);
      await expect(
        provider.open({ ...context, vaultId: "other" }),
      ).rejects.toThrow("owning Rust admission");
      await expect(
        provider.open({ ...context, role: "proxy" }),
      ).rejects.toThrow("owning Rust admission");
      expect(fetch).toHaveBeenCalledTimes(1);
    } finally {
      await client.close();
    }
  });

  it("snapshots writes and preserves native undefined versus null replies", async () => {
    const calls: Array<{ method: string; args: Array<{ value?: unknown }> }> =
      [];
    const fetch = vi.fn<typeof globalThis.fetch>(async (_url, init) => {
      if (init?.method === "DELETE") return empty();
      const call = JSON.parse(String(init?.body));
      if (call.protocol_version) return Response.json(admission);
      calls.push(call);
      if (call.method === "getMeta" && call.args[0].value === "null")
        return Response.json({ kind: "value", value: null });
      return Response.json({ kind: "undefined" });
    });
    const client = await new RustHttpClient("", fetch).attach("vault");
    const database = await new RustDatabaseProvider(client).open(context);
    try {
      const value = { text: "café 🪨" };
      const write = database.setMeta("key", value);
      value.text = "changed after submission";
      await write;
      expect(calls[0].args[1]).toEqual({
        kind: "value",
        value: { text: "café 🪨" },
      });
      await expect(database.getMeta("missing")).resolves.toBeUndefined();
      await expect(database.getMeta("null")).resolves.toBeNull();
    } finally {
      await database.close();
      await client.close();
    }
  });

  it("drains accepted writes on view close and rejects later calls", async () => {
    let enter!: () => void;
    let release!: () => void;
    const entered = new Promise<void>((resolve) => {
      enter = resolve;
    });
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const fetch = vi.fn<typeof globalThis.fetch>(async (url, init) => {
      if (init?.method === "DELETE") return empty();
      if (!String(url).endsWith("/database")) return Response.json(admission);
      enter();
      await gate;
      return Response.json({ kind: "undefined" });
    });
    const client = await new RustHttpClient("", fetch).attach("vault");
    const database = await new RustDatabaseProvider(client).open(context);
    try {
      const write = database.setMeta("key", "accepted");
      await entered;
      let finished = false;
      const closing = database.close().then(() => {
        finished = true;
      });
      await expect(database.getMeta("late")).rejects.toThrow("closed");
      expect(finished).toBe(false);
      release();
      await write;
      await closing;
      expect(finished).toBe(true);
      expect(
        fetch.mock.calls.filter(([url]) => String(url).endsWith("/database")),
      ).toHaveLength(1);
    } finally {
      release();
      await database.close();
      await client.close();
    }
  });

  it("observes native commit frames and aborts its reader without revision polling", async () => {
    let events!: ReadableStreamDefaultController<Uint8Array>;
    let signal: AbortSignal | undefined;
    const cancelled = vi.fn();
    const fetch = vi.fn<typeof globalThis.fetch>(async (url, init) => {
      if (init?.method === "DELETE") return empty();
      if (String(url).includes("/database/events?")) {
        signal = init?.signal ?? undefined;
        return new Response(
          new ReadableStream({
            start(controller) {
              events = controller;
            },
            cancel: cancelled,
          }),
          { headers: { "content-type": "text/event-stream" } },
        );
      }
      return Response.json(admission);
    });
    const client = await new RustHttpClient("", fetch).attach("vault");
    const database = await new RustDatabaseProvider(client).open(context);
    try {
      const listener = vi.fn();
      const dispose = database.subscribeToChanges(listener);
      await vi.waitFor(() => expect(events).toBeDefined());
      const publish = (value: Record<string, unknown>) =>
        events.enqueue(
          new TextEncoder().encode(
            `event: change\nid: ${value.revision}\ndata: ${JSON.stringify(value)}\n\n`,
          ),
        );
      publish({
        revision: 0,
        domains: [],
        paths: [],
        reset: true,
        committedAt: 1,
      });
      await vi.waitFor(() =>
        expect(listener).toHaveBeenLastCalledWith(
          expect.objectContaining({ revision: 0, reset: true }),
        ),
      );
      publish({
        revision: 7,
        domains: ["metadata"],
        paths: ["Changed.md"],
        committedAt: 2,
      });
      await vi.waitFor(() =>
        expect(listener).toHaveBeenLastCalledWith(
          expect.objectContaining({ revision: 7, paths: ["Changed.md"] }),
        ),
      );
      dispose();
      expect(signal?.aborted).toBe(true);
      await vi.waitFor(() => expect(cancelled).toHaveBeenCalledOnce());
      expect(
        fetch.mock.calls.filter(([url]) => String(url).endsWith("/database")),
      ).toHaveLength(0);
    } finally {
      await database.close();
      await client.close();
    }
  });
});
