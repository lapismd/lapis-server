import { describe, expect, it, vi } from "vitest";
import { createRustHostedBackend } from "./backend";
import { RustHttpClient } from "./client";
async function fixture() {
  const fetch = vi.fn<typeof globalThis.fetch>(async (_url, request) => {
    if (request?.method === "DELETE")
      return new Response(null, { status: 204 });
    if (String(_url).endsWith("/clients"))
      return Response.json({
        client_id: "client",
        vault_id: "vault",
        generation: "gen",
      });
    if ((request?.method ?? "GET") === "GET")
      return new Response("saved native conversation");
    const command = JSON.parse(String(request?.body));
    return Response.json(
      command.kind === "set_enabled"
        ? { kind: "availability", enabled: command.enabled }
        : { kind: "records", records: [] },
    );
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
  const app = {
    workspaceTrust: { trusted: true },
    safeMode: { disableOptionalCorePlugins: false },
    notifications: { notify: vi.fn() },
  };
  return { resources, fetch, app, chat: resources.nativeChat!(app as never) };
}
describe("Rust hosted native chat adapter", () => {
  it("uses one admitted transport and leaves native work running on client detach", async () => {
    const f = await fixture();
    try {
      await f.chat.activate();
      await f.chat.call({ kind: "list" });
      await f.chat.leave("detach");
      expect(
        f.fetch.mock.calls.filter(([url]) => String(url).endsWith("/turns")),
      ).toHaveLength(1);
      expect(f.fetch.mock.calls[1][0]).toBe("/api/v3/clients/client/turns");
      expect(JSON.parse(String(f.fetch.mock.calls[1][1]?.body))).toEqual({
        kind: "list",
      });
    } finally {
      await f.resources.release();
    }
  });
  it("explicit feature disable invokes native fence and drain instead of cancelling a view", async () => {
    const f = await fixture();
    try {
      await f.chat.leave("disable");
      expect(JSON.parse(String(f.fetch.mock.calls[1][1]?.body))).toEqual({
        kind: "set_enabled",
        enabled: false,
      });
      f.app.workspaceTrust.trusted = false;
      expect(f.chat.allowed()).toBe(false);
    } finally {
      await f.resources.release();
    }
  });
  it("reads retained conversation files without introducing a browser writer", async () => {
    const f = await fixture();
    try {
      expect(
        await f.chat.readText(".lapis/agents/sessions/id/transcript.jsonl"),
      ).toBe("saved native conversation");
      expect(f.fetch.mock.calls[1][1]?.method ?? "GET").toBe("GET");
      expect(String(f.fetch.mock.calls[1][0])).toContain(
        "/clients/client/files/.lapis/agents/sessions/id/transcript.jsonl",
      );
    } finally {
      await f.resources.release();
    }
  });
});
