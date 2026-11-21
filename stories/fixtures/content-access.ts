import { RustHttpClient } from "../../web/src/hosted/rust/client";
import type { HostedVault } from "../../web/src/hosted/backend";
import type {
  ContentHostStatus,
  PersonalTokenLifetime,
  PersonalTokenRequest,
  PersonalTokenSummary,
} from "../../crates/lapis-app-protocol/bindings/protocol";

const days = 24 * 60 * 60 * 1000;
const lifetimes: Record<PersonalTokenLifetime, number | null> = {
  hour: days / 24,
  day: days,
  week: 7 * days,
  days30: 30 * days,
  days90: 90 * days,
  year: 365 * days,
  never: null,
};

/** Disposable HTTP fixture; real authorization/credentials are covered by hosted acceptance. */
export function contentFixture(
  vaults: () => HostedVault[],
  sampleTokens = false,
) {
  const now = Date.now();
  const approval: PersonalTokenSummary = {
    grant_id: "demo-token",
    label: "Notes backup",
    vault_ids: ["notes"],
    permissions: [],
    created_at: now - days,
    expires_at: null,
    last_used_at: now - days / 24,
    active: true,
  };
  const personalTokens = sampleTokens ? [approval] : [];
  const connections = sampleTokens
    ? [
        {
          ...approval,
          grant_id: "demo-mcp-connection",
          client_id: "demo-mcp-client",
          label: "Example AI assistant",
          expires_at: now + 89 * days,
        },
      ]
    : [];
  const status: ContentHostStatus = {
    operation_permissions: ["tasks:read", "tasks:write"],
    available: true,
    persistent: true,
    access: { enabled: sampleTokens, grants: [] },
    personal_tokens: personalTokens,
    mcp_connections: connections,
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
        if (!status.access.enabled) {
          status.access.grants = [];
          for (const entry of [...personalTokens, ...connections])
            entry.active = false;
        }
      }
      return Response.json(status);
    }
    if (path.endsWith("/grants") && init?.method === "POST") {
      const input = JSON.parse(String(init.body));
      const grant_id = crypto.randomUUID();
      status.access.grants.push({
        permissions: input.permissions ?? [],
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
    if (path.endsWith("/tokens") && init?.method === "POST") {
      const input: PersonalTokenRequest = JSON.parse(String(init.body));
      const grant_id = crypto.randomUUID();
      const created_at = Date.now();
      const lifetime = lifetimes[input.lifetime];
      personalTokens.unshift({
        grant_id,
        label: input.label,
        vault_ids: input.vault_ids,
        permissions: input.permissions,
        created_at,
        expires_at: lifetime === null ? null : created_at + lifetime,
        last_used_at: null,
        active: true,
      });
      return Response.json(
        { grant_id, token: `lp1_${"d".repeat(43)}` },
        { status: 201 },
      );
    }
    if (init?.method === "DELETE") {
      const id = path.split("/").pop();
      const entry = [...personalTokens, ...connections].find(
        (item) => item.grant_id === id,
      );
      if (entry) entry.active = false;
      status.access.grants = status.access.grants.filter(
        (g) => g.grant_id !== id,
      );
      return new Response(null, { status: 204 });
    }
    return Response.json(
      { code: "NOT_FOUND", message: "Unknown fixture route" },
      { status: 404 },
    );
  });
}
