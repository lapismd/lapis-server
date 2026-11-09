import { RustHttpClient } from "../../web/src/hosted/rust/client";
import type { HostedVault } from "../../web/src/hosted/backend";
import type { ContentHostStatus } from "../../crates/lapis-app-protocol/bindings/protocol";

/** Disposable HTTP fixture; real authorization/credentials are covered by hosted acceptance. */
export function contentFixture(vaults: () => HostedVault[]) {
  const status: ContentHostStatus = {
    available: true,
    persistent: true,
    access: { enabled: false, grants: [] },
    api_url: "https://notes.example/api/content/v1",
    mcp_url: "https://notes.example/mcp",
  };
  return new RustHttpClient("https://notes.example", async (url, init) => {
    const path = new URL(String(url)).pathname;
    if (path.endsWith("/vaults"))
      return Response.json(
        vaults().map((v) => ({
          vault_id: v.id,
          name: v.name,
          archived_at: v.archivedAt,
        })),
      );
    if (path.endsWith("/content-access")) {
      if (init?.method === "POST") {
        status.access.enabled = JSON.parse(String(init.body)).enabled;
        if (!status.access.enabled) status.access.grants = [];
      }
      return Response.json(status);
    }
    if (path.endsWith("/grants") && init?.method === "POST") {
      const input = JSON.parse(String(init.body));
      const grant_id = crypto.randomUUID();
      status.access.grants.push({
        grant_id,
        label: input.label,
        vault_ids: input.vault_ids,
        scope: "read",
        active: true,
        expires_in_seconds: input.lifetime_seconds,
      });
      return Response.json(
        { grant_id, token: `lc1_${"a".repeat(43)}` },
        { status: 201 },
      );
    }
    if (init?.method === "DELETE") {
      status.access.grants = status.access.grants.filter(
        (g) => g.grant_id !== path.split("/").pop(),
      );
      return new Response(null, { status: 204 });
    }
    return Response.json(
      { code: "NOT_FOUND", message: "Unknown fixture route" },
      { status: 404 },
    );
  });
}
