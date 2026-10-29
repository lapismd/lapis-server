import { describe, expect, it, vi } from "vitest";
import { createRustHostedBackend } from "./backend";
import { RustHttpClient } from "./client";

describe("retained chooser over Rust", () => {
  it("uses the selected host management capability and retains archive state", async () => {
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValueOnce(
        Response.json({
          protocol_major: 3,
          build_id: "test",
          browser_auth: true,
          manage_vaults: true,
          transfer_vaults: false,
        }),
      )
      .mockResolvedValueOnce(
        Response.json([{ vault_id: "id", name: "Archived", archived_at: 123 }]),
      );
    const backend = createRustHostedBackend(new RustHttpClient("", fetch));
    await backend.info();
    expect(backend.capabilities.manageVaults).toBe(true);
    expect(backend.capabilities.transferVaults).toBe(false);
    expect(await backend.vaults()).toEqual([
      { id: "id", name: "Archived", role: "owner", archivedAt: 123 },
    ]);
  });
  it("enables retained transfer controls and uploads bytes through the selected Rust host", async () => {
    const imported = Response.json(
      { vault_id: "id", name: "Café notes" },
      { status: 201 },
    );
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValueOnce(
        Response.json({
          protocol_major: 3,
          build_id: "test",
          browser_auth: true,
          manage_vaults: true,
          transfer_vaults: true,
        }),
      )
      .mockResolvedValueOnce(
        Response.json({ username: "owner", csrf_token: "csrf", expires_at: 1 }),
      )
      .mockResolvedValueOnce(imported);
    const backend = createRustHostedBackend(
      new RustHttpClient("https://notes.test", fetch),
    );
    await backend.info();
    await backend.current();
    expect(backend.capabilities.transferVaults).toBe(true);
    const archive = new Blob([new Uint8Array([80, 75])], {
      type: "application/zip",
    });
    await backend.importArchive("Café notes", archive);
    expect(imported.bodyUsed).toBe(true);
    const [url, request] = fetch.mock.calls[2];
    expect(url).toBe(
      "https://notes.test/api/v3/vaults/import?name=Caf%C3%A9%20notes",
    );
    expect(request?.method).toBe("POST");
    expect(request?.body).toBe(archive);
    expect(new Headers(request?.headers).get("x-lapis-csrf")).toBe("csrf");
    expect(backend.exportUrl("id")).toBe(
      "https://notes.test/api/v3/vaults/id/export",
    );
  });
  it("routes the existing create, rename, archive and restore actions without a Node fallback", async () => {
    const fetch = vi.fn<typeof globalThis.fetch>(async () =>
      Response.json({ vault_id: "id", name: "Notes" }),
    );
    const backend = createRustHostedBackend(new RustHttpClient("", fetch));
    expect(await backend.create("Notes")).toEqual({ vaultId: "id" });
    await backend.rename("id", "Renamed");
    await backend.archive("id");
    await backend.restore("id");
    expect(fetch.mock.calls.map(([url]) => url)).toEqual([
      "/api/v3/vaults",
      "/api/v3/vaults/id/name",
      "/api/v3/vaults/id/archive",
      "/api/v3/vaults/id/restore",
    ]);
    expect(fetch.mock.calls.map(([, init]) => init?.method)).toEqual([
      "POST",
      "POST",
      "POST",
      "POST",
    ]);
    expect(JSON.parse(String(fetch.mock.calls[0][1]?.body))).toEqual({
      name: "Notes",
    });
    expect(JSON.parse(String(fetch.mock.calls[1][1]?.body))).toEqual({
      name: "Renamed",
    });
  });
});
