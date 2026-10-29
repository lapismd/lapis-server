import { describe, expect, it, vi } from "vitest";
import { createRustHostedBackend } from "./backend";
import { RustHttpClient } from "./client";
const selection = {
  recipient: {
    id: "writer",
    label: "Writer",
    token: "jj-store-agent:777269746572;",
  },
  target: { id: "writer", agent: "codex", thinking: "default" },
};
async function fixture(declared = true) {
  const fetch = vi.fn<typeof globalThis.fetch>(async (url, request) => {
    if (request?.method === "DELETE")
      return new Response(null, { status: 204 });
    if (String(url).endsWith("/clients"))
      return Response.json({
        client_id: "client",
        vault_id: "vault",
        generation: "gen",
      });
    const command = JSON.parse(String(request?.body));
    if (String(url).endsWith("/queues"))
      return Response.json({ kind: "queue", queue: null });
    return Response.json(
      command.kind === "targets"
        ? {
            kind: "targets",
            targets: [
              {
                id: "opaque",
                label: "Display name",
                default_mode: "read-only",
                ...(declared ? { queue_selection: selection } : {}),
              },
            ],
          }
        : { kind: "availability", enabled: true },
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
  const chat = resources.nativeChat!({
    workspaceTrust: { trusted: true },
    safeMode: { disableOptionalCorePlugins: false },
  } as never);
  return { resources, chat, fetch };
}
describe("hosted native queue selection", () => {
  it("reads the complete operator-declared capture separately from opaque target identity", async () => {
    const f = await fixture();
    try {
      expect(await f.chat.queue!.selection("opaque")).toEqual(selection);
      expect(await f.chat.queue!.selection("missing")).toBeUndefined();
      expect(
        await f.chat.queue!.call({
          kind: "read",
          conversation_id: "conversation",
        }),
      ).toEqual({ kind: "queue", queue: null });
      expect(f.fetch.mock.calls.at(-1)?.[0]).toBe(
        "/api/v3/clients/client/queues",
      );
    } finally {
      await f.resources.release();
    }
  });
  it("leaves old R2 targets unavailable for queue capture without inferring from their labels", async () => {
    const f = await fixture(false);
    try {
      expect(await f.chat.queue!.selection("opaque")).toBeUndefined();
      expect(
        f.fetch.mock.calls.filter(([url]) => String(url).endsWith("/queues")),
      ).toHaveLength(0);
    } finally {
      await f.resources.release();
    }
  });
});
