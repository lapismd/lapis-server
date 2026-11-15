# Lapis Sync arrival catch-up — 2026-11-15

This fourth clean Server Core slice extends the guarded object, operation and
event repositories at `d124bb07`. It retains arrival order and caller-validated
opaque membership history. It does not duplicate Plugins-owned protocol,
signature, attestation, owner-authority or cutoff validation.

## Exact unavailable boundary

The new target initially failed before collection because `ArrivalRepository`,
`ArrivalKind`, `ArrivalCursor` and `CatchUpOutcome` did not exist:

```sh
cargo +1.99.0 test -p lapis-app-core \
  --test sync_arrival_repository --locked -- --nocapture
```

The completed exact target passes **8/8**. The combined object, operation
journal, event-ordering and arrival cohort passes **28/28**.

## Implemented boundary

One exclusive `ArrivalRepository` writer is bound to an admitted physical
profile, logical vault and explicit destination. Revision and HEAD arrivals are
recorded only after the corresponding caller-validated event and dependencies
are durable in `EventRepository`. A monotonic local sequence, rather than an
event timestamp or identity, defines catch-up order. Exact retries reuse one
sequence; changed kind, membership metadata or bytes fail closed.

Each new catch-up pins the then-current sequence as a stable snapshot. Bounded
pages exclude later arrivals until the caller starts a fresh catch-up. Cursor
generation expiry returns `RestartRequired`, while missing or corrupt records
return explicit errors. Empty pages are complete observations and never a
deletion signal.

Caller-validated membership events retain their exact opaque sealed bytes,
SHA-256 integrity, generation and predecessor. The chain is append-only and
gap-free. A crash after publishing the immutable sequence record but before
identity/state discovery is repaired on cold reopen without allocating a new
sequence or replaying changed bytes.

The exact cases cover stable snapshots with later arrivals, immutable
membership bytes and chains, exact retries, empty pages, cursor expiry,
exclusive ownership, cold reopen, interrupted publication recovery, missing
records, invalid bounds and replaced profiles. Every production file remains
below 300 lines and the exact test remains below 500.

## Qualification

- Exact arrival target: **8 passed, 0 failed**. Combined Sync repository
  cohort: **28 passed, 0 failed**.
- Complete `lapis-app-core`: **601 passed, 0 failed, 7 ignored**. The complete
  Rust workspace: **836 passed, 0 failed, 7 ignored**. Every ignore is an
  existing governed native-asset or isolated-process helper; no Sync case is
  ignored.
- Strict affected and complete-workspace all-target Clippy pass with warnings
  denied. The retained macOS compact-unwind linker warning is non-failing.
- Node **24.10.0** `pnpm spec:check`, `pnpm check`, `pnpm test` and `pnpm build`
  pass. CLI distribution passes **4/4**, the web unit lane passes **132/132**,
  Svelte reports zero errors and warnings, and the App-ownership audit passes.
  The first post-rebase `pnpm check` found the isolated clone's stale
  pre-rebase install missing `@lapis-notes/ui/cli`; a frozen-lockfile install
  refreshed the three Notes dependencies selected by current `main`, after
  which the exact check and wider Node lanes passed.
- The first specification attempt rejected a fifth SRV-SYNC-004 acceptance
  bullet; the requirement was moved into its normative paragraph. The second
  attempt required the protected source mapping in the hosted-server chapter.
  The final validate, mdBook and spec-first lanes pass, and QMD-backed search
  plus `spec-validator doctor --json` are healthy under Node 24.
- Production modules are **226**, **176** and **163** lines; the exact test is
  **225** lines.

## Unclaimed boundaries

This repository does not validate signatures, attestations, membership owner
authority, revocation cutoffs or content keys. It does not expose HTTP, page
object bodies, schedule work, mutate a working vault, enroll a server
participant or select Sync for an application. Independent devices, hosted
images, Desktop custody and installed applications remain unqualified.
