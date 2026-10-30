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
