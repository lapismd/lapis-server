# Shared content HTTP reads

Requirements: SRV-SRV-004/009. Parent credential slice: `1836958f`, PR #22.

The private `lapis-content-http` crate exposes one reusable read-only router
over shared Core grants. It owns wire decoding, Host/Origin/version checks,
bounded blocking dispatch, typed control/error mapping and direct note bytes
with strong ETags. Neither production host mounts the router in this slice.

## Validation

Environment: Linux, Rust 1.99.0, Node 24.19.0 and system JJ 0.46.0.
CI retains its independent pinned JJ 0.45.1 and hosted/image gates.

- Before implementation, `cargo test -p lapis-content-http --test reads
--offline` failed with one unresolved-import diagnostic for the missing router,
  route inventory and header constants. The new workspace member resolved only
  already-locked dependency versions while creating its lockfile entry.
- The same real-HTTP integration target with `--locked` covers Core catalog
  parity, exact UTF-8 bytes/versions, lists/cursors, credentials, revocation,
  Host/Origin, protocol, strict queries, limits and closed-vault refusal.
- Private unit cases cover configured origins, duplicate credential/authority
  headers, native saturation and caller cancellation retaining worker capacity.
- An added URI-boundary regression failed 1/1: a 16 KiB + 1 request returned
  semantic 400 instead of transport 413 because nested routing removed the
  prefix before measuring. The adapter now uses Axum's retained original URI;
  the exact failing case then passed 1/1 before final workspace validation.

The initial integration command with `--offline` now passes 6/6; the adapter
crate with `--locked` passes all 10 cases. The final `cargo test --workspace
--locked` run passes 735 tests, with 0 failures and 7 pre-existing model/helper
ignores. Strict workspace Clippy, protocol drift/type checks, configuration,
specification, Prettier and edited-file Rustfmt pass. No existing dependency
version or upstream Git pin changed; Axum's original-URI feature was enabled.

## Remaining boundaries

These are real-socket shared-adapter tests, not production Server or Desktop
acceptance. Host mounting/consent, persisted credentials, remote OAuth, writes
and durable receipts, CLI/MCP adapters and platform acceptance remain in the
content backlog. No new ignore or assertion waiver is introduced for this work.
