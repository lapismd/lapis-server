import { describe, expect, it, vi } from "vitest";
import { createRustHostedBackend } from "./backend";
import { RustHttpClient } from "./client";

async function fixture(outcome: (command: unknown) => unknown) {
  const fetch = vi.fn<typeof globalThis.fetch>(async (url, init) => {
    if (String(url).endsWith("/auth/current"))
      return Response.json({ csrf_token: "csrf" });
    if (init?.method === "DELETE") return new Response(null, { status: 204 });
    if (String(url).endsWith("/clients"))
      return Response.json({
        client_id: "client",
        vault_id: "vault",
        generation: "vault-generation",
      });
    const request = JSON.parse(String(init?.body));
    return Response.json({
      protocol_version: 3,
      request_id: request.request_id,
      outcome: outcome(request.command),
    });
  });
  const host = new RustHttpClient("https://notes.test", fetch);
  await host.current();
  const resources = await createRustHostedBackend(host, {
    nativeTasks: true,
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

describe("Rust hosted editing-intent binding", () => {
  it("uses the admitted client command envelope and never supplies a second owner", async () => {
    const value = { kind: "missing" };
    const f = await fixture(() => ({
      status: "ok",
      value: { kind: "editing_intents", value },
    }));
    try {
      expect(f.resources.nativeEditingIntents).toBeDefined();
      const command = {
        kind: "inspect" as const,
        editor_id: "d5ab7890-0000-4000-8000-000000000001",
      };
      expect(await f.resources.nativeEditingIntents!.call(command)).toEqual(
        value,
      );
      const [url, init] = f.fetch.mock.calls[2]!;
      expect(url).toBe("https://notes.test/api/v3/clients/client/commands");
      expect(new Headers(init?.headers).get("x-lapis-csrf")).toBe("csrf");
      expect(JSON.parse(String(init?.body))).toEqual({
        protocol_version: 3,
        request_id: expect.any(String),
        vault_id: "vault",
        generation: "vault-generation",
        command: { kind: "editing_intents", command },
      });
      await f.resources.release();
      await expect(
        f.resources.nativeEditingIntents!.call(command),
      ).rejects.toThrow("closed");
      expect(f.fetch).toHaveBeenCalledTimes(4);
    } finally {
      await f.resources.release();
    }
  });

  it("returns the exact read snapshot and maps only a typed missing file to null", async () => {
    const snapshot = {
      path: "Task.md",
      text: "# Task\n",
      version: "a".repeat(64),
    };
    const f = await fixture((command) =>
      (command as { path: string }).path === "Task.md"
        ? { status: "ok", value: { kind: "text", snapshot } }
        : { status: "error", error: { code: "NOT_FOUND", message: "Missing" } },
    );
    try {
      expect(f.resources.nativeEditingIntents).toBeDefined();
      expect(await f.resources.nativeEditingIntents!.read("Task.md")).toEqual(
        snapshot,
      );
      expect(
        await f.resources.nativeEditingIntents!.read("Gone.md"),
      ).toBeNull();
      expect(
        JSON.parse(String(f.fetch.mock.calls[2]![1]?.body)).command,
      ).toEqual({ kind: "read_text", path: "Task.md" });
    } finally {
      await f.resources.release();
    }
  });

  it.each(["PERMISSION_DENIED", "IO"])(
    "retains %s and never reports it as a missing file",
    async (code) => {
      const f = await fixture(() => ({
        status: "error",
        error: { code, message: "Read refused" },
      }));
      try {
        expect(f.resources.nativeEditingIntents).toBeDefined();
        await expect(
          f.resources.nativeEditingIntents!.read("Task.md"),
        ).rejects.toMatchObject({ code });
        expect(f.fetch).toHaveBeenCalledTimes(3);
      } finally {
        await f.resources.release();
      }
    },
  );
});
