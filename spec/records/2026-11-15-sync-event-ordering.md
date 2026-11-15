# Lapis Sync event and HEAD ordering — 2026-11-15

This third clean Server Core slice extends the guarded object repository and
operation journal at `06698ee8`. It stores only opaque caller-validated signed
event bytes and their admitted dependency metadata. It does not implement or
duplicate Plugins-owned protocol, signature, attestation, membership or content
key validation.

## Exact unavailable boundary

The new target initially failed before collection because `EventRepository` and
`EventKind` did not exist:

```sh
cargo +1.99.0 test -p lapis-app-core \
  --test sync_event_repository --locked -- --nocapture
```

The completed exact target passes **8/8**. The combined object, operation
journal and event-ordering cohort passes **20/20**.

## Wider qualification

- `cargo +1.99.0 test -p lapis-app-core --locked` passes **593**, with **7**
  governed native-asset/process-helper ignores and **0** failures.
- The final `cargo +1.99.0 test --workspace --locked --quiet` repetition passes
  **820**, with the same **7** governed ignores and **0** failures.
- The first count-only workspace attempt reported one unlabelled Core library
  failure after **150** passes and one ignore. The immediately repeated exact
  library boundary, the complete workspace rerun, and two additional exact
  library repetitions all passed **151/151** with the one governed helper
  ignore. The aggregate attempt did not retain the failed test name, so this
  transient attempt remains recorded rather than being treated as qualified
  evidence.
- Node **24.10.0** `pnpm check`, `pnpm test` and `pnpm build` pass; the web unit
  lane passes **132/132**. `pnpm spec:check`, strict affected/full-workspace
  Clippy and Rust formatting pass. The production modules are **228**, **169**
  and **147** lines; the exact test is **385** lines.

## Implemented boundary

`EventRepository` is bound to one admitted physical profile, logical vault and
explicit destination. Its public write methods document that the Plugins-owned
validator must first authenticate signed event identity, authority and the
complete dependency list. The repository independently enforces bounded opaque
bytes, lowercase event/object identities, durable metadata scope and immutable
exact retry reuse.

A revision event becomes readable only after all encrypted objects and parent
revision events exist and pass storage integrity checks. A HEAD event becomes a
discoverable device tip only after its complete bounded revision graph and all
transitive object dependencies pass those checks. Missing or corrupt
dependencies return explicit errors; they never become an empty side.

Each device has an independent conditional HEAD pointer. The signed HEAD event
is durable before that pointer changes. A stale expected pointer leaves the new
event recoverable but undiscoverable, and an exact lost-acknowledgement retry
reuses both event and pointer. Concurrent identical event/HEAD publication
converges on one durable identity; a foreign concurrent value is never
overwritten.

The exact cases cover missing objects and parents, exact retries, independent
device tips, cold reopen, stale conditional writes, corrupt transitive objects,
corrupt events and pointers, replaced profiles, concurrent publication, invalid
authority-shaped input and record/graph bounds. Every production file remains
below 300 lines and the exact test remains below 500.

## Unclaimed boundaries

The native repository does not decide whether opaque bytes are a valid signed
protocol event. Plugins-owned validation and host admission must occur before
calling it and again before applying downloaded data. Membership history,
revocation/cutoff decisions, paging, HTTP, scheduling, working-file apply,
Desktop custody, hosted images, independent devices and installed applications
remain unqualified.
