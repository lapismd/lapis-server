# Lapis application core

This repository owns the transport-independent native application services used
by the Rust host and Tauri consumers. Read the [current server specification](../../spec/src/current/index.md)
and [native-service backlog](../../spec/src/backlog/native-services.md) for
supported boundaries and outstanding work. Shared public App/API requirements
remain in the [Notes specification](https://lapismd.github.io/lapis-notes/).

The vault engine supplies checked relative-path IO, bounded reads, atomic and
conditional publication and exclusive mutations. Hosts admit physical roots and
retain OS/window policy. Native capability handles do not become renderer or
HTTP authority.

Service owners retain admitted clients, serialized writers, dirty-editor claims,
observation and fenced drain. Accepted work and durable receipts have their
explicit lifecycle contracts; detaching an observer does not grant another writer
or authorize replay. The protocol crate owns portable commands and generated
client contracts. Database work uses admitted private storage and a bounded
worker; clients do not receive raw SQL or native path authority.

Optional sibling projection implementations are selected explicitly through
local-sibling-projections. The ordinary workspace does not acquire external
Cargo path dependencies; unavailable projection/task capabilities report their
supported state. Source-level feature implementations and application/provider
qualification remain separate. The owning backlog records remaining integration
and protocol/architecture-tooling reconciliation.

Run cargo test --workspace --locked and cargo clippy --workspace --all-targets
--locked -- -D warnings from this repository root, following its agent guide and
CI prerequisites. The retired Notes pnpm Rust wrappers and migration plan are
not current commands or completion authority.

Core application policy has no Tauri, HTTP, renderer or browser dependency.
Consumers use public modules directly instead of copying engines or routing
through old Desktop implementation paths.

`services::content::ContentGrants` provides bounded, process-local read-only
credentials over dedicated admitted clients. Production hosts construct
`ContentGrants::disabled()`, authorize owner consent, then call `set_enabled(true)`.
`Default` retains the enabled low-level fixture mode. Hosts issue named credentials
with `issue_named_read_only`, display secret-free `status()` summaries, revoke by
grant ID, and schedule `prune_expired` on blocking workers. Tokens are revealed
only by the issuance result; a restart requires new credentials.

`set_enabled(false)` fences every grant before drain; re-enable cannot restore
old tokens or an issuance started before disabling. `revoke_vault` revokes whole
grants containing a retiring vault and invalidates pending issuance. Hosts must
serialize native vault admission against this retirement. `shutdown` permanently
closes issuance and drains accepted reads. All native control calls can block;
hosts retain accepted-work ownership after client cancellation.

[SRV-SRV-013](../../spec/src/current/content.md#srv-srv-013) owns these native
controls. This library does not mount endpoints, persist settings/consent or
register plugin screens. Both hosts' integration gates remain in the
[content backlog](../../spec/src/backlog/content-automation.md).

For a debug-only backup initialization investigation, explicitly set
`LAPIS_APP_BACKUP_DIAGNOSTICS=1`. Each initialization reports a bounded set of
fixed stage/outcome/category records, process and attempt IDs, elapsed time and
an OS error code where the owning error type retains it. Paths, error text,
credentials and vault content are excluded; public errors remain sanitized.
Release builds do not emit these diagnostics. A stage report identifies where
initialization failed; it does not establish or repair its cause.
