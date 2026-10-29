import { describe, expect, it, vi } from "vitest";
import { RustHttpClient } from "./client";
import { createRustHostedBackend } from "./backend";

const admission = { client_id: "client", vault_id: "vault", generation: "gen" };
describe("hosted native participant transport", () => {
  it("does not deliver a participant reply after its admitted client retires", async () => {
    let release!: (response: Response) => void;
    const fetch = vi.fn<typeof globalThis.fetch>(async (url, request) => {
      if (request?.method === "DELETE")
        return new Response(null, { status: 204 });
      if (String(url).endsWith("/clients")) return Response.json(admission);
      return new Promise((resolve) => {
        release = resolve;
      });
    });
    const client = await new RustHttpClient("", fetch).attach("vault");
    const pending = client.participants({ kind: "profiles" });
    const rejected = expect(pending).rejects.toThrow("closed");
    await Promise.resolve();
    await client.close();
    release(Response.json({ kind: "profiles", profiles: [] }));
    await rejected;
    expect(fetch).toHaveBeenCalledTimes(3);
  });
  it("forwards exact native lookup through the admitted CSRF port and fences after detach", async () => {
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValueOnce(Response.json({ csrf_token: "session-csrf" }))
      .mockResolvedValueOnce(Response.json(admission))
      .mockResolvedValueOnce(Response.json({ kind: "message", message: null }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    const host = new RustHttpClient("", fetch);
    await host.current();
    const client = await host.attach("vault");
    const command = {
      kind: "for_turn" as const,
      request_id: crypto.randomUUID(),
    };
    try {
      expect(await client.participants(command)).toEqual({
        kind: "message",
        message: null,
      });
      const [path, request] = fetch.mock.calls[2];
      expect(path).toBe("/api/v3/clients/client/participants");
      expect(new Headers(request?.headers).get("x-lapis-protocol")).toBe("3");
      expect(new Headers(request?.headers).get("x-lapis-csrf")).toBe(
        "session-csrf",
      );
      expect(JSON.parse(String(request?.body))).toEqual(command);
    } finally {
      await client.close();
    }
    await expect(client.participants(command)).rejects.toThrow("closed");
    expect(fetch).toHaveBeenCalledTimes(4);
  });
  it("leaves a lost acceptance untouched and reads its original identity without resubmission", async () => {
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValueOnce(Response.json(admission))
      .mockRejectedValueOnce(new TypeError("participant reply lost"))
      .mockResolvedValueOnce(Response.json({ kind: "message", message: null }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    const client = await new RustHttpClient("", fetch).attach("vault");
    const input = {
      acceptance_id: crypto.randomUUID(),
      user: { id: "authored-parent" },
      dispatch: { id: "dispatch" },
      turns: [],
    };
    try {
      await expect(
        client.participants({ kind: "accept", input }),
      ).rejects.toThrow("participant reply lost");
      expect(fetch).toHaveBeenCalledTimes(2);
      await client.participants({
        kind: "read",
        acceptance_id: input.acceptance_id,
      });
      expect(
        fetch.mock.calls
          .slice(1)
          .map(([, request]) => JSON.parse(String(request?.body))),
      ).toEqual([
        { kind: "accept", input },
        { kind: "read", acceptance_id: input.acceptance_id },
      ]);
    } finally {
      await client.close();
    }
  });
  it("exposes declared logical metadata through the existing shared native host", async () => {
    const profiles = [
      {
        target_id: "opaque-runtime",
        selection: {
          recipient: {
            id: "writer",
            label: "Writer",
            token: "jj-store-agent:777269746572;",
          },
          target: { id: "writer", agent: "codex", thinking: "default" },
        },
        options: {
          mode: "plan",
          model: null,
          effort: null,
          max_duration_ms: 60000,
        },
      },
    ];
    const fetch = vi.fn<typeof globalThis.fetch>(async (url, request) => {
      if (request?.method === "DELETE")
        return new Response(null, { status: 204 });
      if (String(url).endsWith("/clients")) return Response.json(admission);
      if (String(url).endsWith("/participants"))
        return Response.json({ kind: "profiles", profiles });
      return Response.json({ kind: "availability", enabled: true });
    });
    const resources = await createRustHostedBackend(
      new RustHttpClient("", fetch),
    ).prepare({
      id: "vault",
      name: "Notes",
      kind: "server-hosted",
      role: "owner",
      archived: false,
      createdAt: 0,
      updatedAt: 0,
    });
    try {
      const chat = resources.nativeChat!({
        workspaceTrust: { trusted: true },
        safeMode: { disableOptionalCorePlugins: false },
      } as never);
      expect(await chat.participants!.call({ kind: "profiles" })).toEqual({
        kind: "profiles",
        profiles,
      });
      expect(fetch.mock.calls.at(-1)?.[0]).toBe(
        "/api/v3/clients/client/participants",
      );
    } finally {
      await resources.release();
    }
  });
});
