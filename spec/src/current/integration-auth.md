# Integration authorization

## SRV-SRV-017 — Persistent personal access tokens

**Requirement.** A profile-backed Server MUST issue owner-managed personal tokens
for explicitly selected vaults and approved content/plugin operations. The local
control database MUST retain only a verifier and the approval record. Active
tokens MUST survive restart, with fresh native leases. Token revocation, disabling
access, and explicitly closing or archiving a selected vault MUST durably revoke
affected tokens. Personal tokens MUST NOT authorize server owner routes.

### Acceptance details

- Issuance reveals the secret once and offers 1 hour, 24 hours, 7 days, 30 days, 90 days by default, 365 days, and Never.
- Fresh leases revalidate vault admission and the original operation snapshot after restart or idle cleanup.
- Disablement, revocation, expiry, or explicit vault retirement permanently end the approval.
- Credential settings use a searchable multi-vault picker and issued credential lists show label, scope, status and absolute expiry without secrets.

## SRV-SRV-018 — OAuth for remote MCP clients

**Requirement.** Profile-backed HTTPS Server MUST advertise OAuth discovery for
MCP and authorize clients through Lapis owner login and explicit vault consent.
MCP tokens MUST be bound to the configured resource and approved operation
snapshot. Durable disconnection, disablement and vault retirement MUST revoke
access and renewal. Initialized Streamable HTTP MUST require bearer authorization on every request.

### Acceptance details

- Client registration, issuer discovery, S256 PKCE and exact callback validation support ChatGPT, Claude and Codex.
- Access expires after 15 minutes and refresh credentials rotate within a 90-day authorization, with family revocation on replay.
- Resource discovery advertises the required read scope and registered operation scopes, with token renewal listed by OAuth discovery and initialized MCP advertising a public PNG logo.
- Owners select vaults and optional operation scopes through searchable multi-select consent controls and can disconnect approved clients from Content API and MCP settings.
