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

`services::content::ContentGrants` provides bounded, ephemeral read-only test
credentials over dedicated admitted clients. Native hosts authorize issuance,
schedule `prune_expired` and call `shutdown` on blocking workers. This library
does not mount endpoints or implement persistent consent/credential custody;
those integration gates remain in the content automation backlog.

For a debug-only backup initialization investigation, explicitly set
`LAPIS_APP_BACKUP_DIAGNOSTICS=1`. Each initialization reports a bounded set of
fixed stage/outcome/category records, process and attempt IDs, elapsed time and
an OS error code where the owning error type retains it. Paths, error text,
credentials and vault content are excluded; public errors remain sanitized.
Release builds do not emit these diagnostics. A stage report identifies where
initialization failed; it does not establish or repair its cause.
