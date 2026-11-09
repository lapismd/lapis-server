# Shared content HTTP adapter

This private workspace crate provides `router(Arc<ContentGrants>, public_origin)`
for controlled Server/Desktop integration. It does not start a listener, issue
credentials, select vault roots or mount itself in either production host.
Read [SRV-SRV-009](../../spec/src/current/content.md#srv-srv-009) and the
[content backlog](../../spec/src/backlog/content-automation.md) for the contract
and remaining Desktop, connection and mutation work. The hosted server mounts
this adapter under [SRV-SRV-014](../../spec/src/current/content.md#srv-srv-014)
when enabled in its Content API and MCP plugin settings.

The router nests four GET routes under `/api/content/v1`: `capabilities`,
`vaults`, `vaults/{vault_id}/notes` and `vaults/{vault_id}/note?path=...`.
Lists accept `limit` and `cursor`; note lists also accept `directory`. Clients
supply a scoped bearer credential and `x-lapis-content-api: 1`. Control replies
use Protocol's `ContentReply`; note reads return UTF-8 bytes and a strong ETag.
The optional `x-lapis-request-id` UUID is echoed in every adapter response.

The host must configure an exact HTTPS origin (HTTP only for loopback), preserve
the matching Host header through its trusted proxy, and own TLS, listener and
connection limits. The router rejects foreign Origin/Host values, never trusts
forwarding headers and never authorizes browser cookies or owner-wide tokens.
It limits native concurrency and retains permits after request cancellation.
`Boundary::new(grants, origin)` and `router_with_boundary(boundary.clone())`
allow HTTP and MCP to share that policy and one native worker pool.

Native hosts authorize named grant issuance, schedule expiry cleanup and shut
down/drain grants on blocking workers. The hosted server persists its enable
preference, while credentials remain process-local. Desktop qualification,
persistent client grants, OAuth and general client compatibility remain pending.

Run `cargo test -p lapis-content-http --locked` for real-socket read/error cases
and native-admission/cancellation tests. Workspace tests and Clippy remain the
repository acceptance lanes.
