# Lapis application core

Transport-independent native application services for Lapis Notes. The first
slice extracts the existing Desktop vault file engine; Tauri consumes this crate
directly. The service layer adds admitted ownership and independent client/work
lifetimes; hosted adapters and durable ACP jobs remain later gates in the
[migration plan](../../RUST_CORE_MIGRATION_PLAN.md).

`vault` owns checked relative-path IO, bounded reads, atomic and conditional
publication, exclusive create/rename/copy and native action targets. Hosts still
own root admission and mutation serialization. Native capability handles never
cross into renderer or HTTP values.

`services` owns one writer per physical vault, conditional text saves, dirty editor
claims, detach/recovery and fenced drain. Hosts derive caller authority and attach
clients; a work lease survives client detachment but is not a durable job receipt.
`lapis-app-protocol` supplies the common commands and generated client contracts.

`database` supplies high-level metadata get/set, prefix scans, conditional
transactions, change revisions, notebook state, notifications and file-history
storage. An installation
selects a private database directory and admitted vault UUID; no operation accepts
SQL or a path. One bounded worker owns `<uuid>.db`, retaining the existing Turso
metadata/change tables and JSON comparison order. Clones share admission; a
cancelled waiter does not cancel an accepted transaction. Close fences all clones,
drains admitted work, checkpoints and releases its native ownership lock.

Native change receivers observe commits without owning them. Their 128-event
buffer reports lag explicitly; path detail above 64 KiB or 500 paths becomes a
complete-domain reset. Notification updates preserve durable read/clear state
and bulk clearing uses one revision. Deprecated metadata-snapshot access remains
an explicit compatibility operation, never a startup or normalized-index fallback.

File-history methods preserve the existing database-only revisions and IDs,
deduplication, rename/delete/restore, coalescing and retention. Their mutations
use the same transaction/change owner. This storage contract is separate from
the JJ repository engine, backup capture and repository scheduling.

`database::indexed` publishes core per-file metadata independently of plugin
projections. Normalized columns support typed filters/sorts, selective domain
materialization in 400-path batches, bounded manifests, facets and link queries.
Large prefix sets use bound value rows to keep native expression depth bounded.
Core publication is a processor boundary; the full AppDatabase upsert still
needs its separate projection stage, and rename/delete retain pending cross-domain
integration.

`database::projections` retains namespaced definition registration, atomic
source replacement and error/delete/unregister cleanup. Rows, typed indexed
values, relationships and post-commit events share the native worker. Shared
browser/native fixtures check scalar conversion and limits. Single-row reads
retain identity/path aliases, explicit private-owner access and source
hash/schema/config freshness without writing. Projection AST/relationship
queries, task composition and background scheduling remain later catalogue work.

This foundation does not migrate the complete AppDatabase schema or connect a
browser/desktop database adapter yet. Remaining catalogue methods and service
intake gate the hosted shell; native indexing/Search remain R3.

Run `pnpm test:rust-core`, `pnpm check:rust-core` and `pnpm build:rust-core` from
the Notes root. Each checkout owns `.local/rust-core-target`; the build wrapper
refuses a target override and requires 30 GiB free. No cleanup is automatic.

Application code has no Tauri, HTTP, renderer or browser dependency. Use the
public module directly; consumers must not copy this engine or re-export its
old Desktop paths.
