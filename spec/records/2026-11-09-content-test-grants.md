# Ephemeral content test grants

Requirement: SRV-SRV-008. Parent native read slice: `afdbe8ce`, PR #20.

Core now owns bounded read-only test credentials over dedicated native sessions.
Issuance returns an explicitly accessed, Debug-redacted random bearer token;
the registry retains only a verifier. Credential dispatch reuses the original
session for vault policy, reads, cursors and native-lock revalidation. Revocation,
expiry cleanup and shutdown fence authority and drain before releasing capacity.

## Validation

Environment: Linux, Rust 1.99.0, Node 24.19.0 and system JJ 0.46.0.
CI retains its independent pinned JJ 0.45.1 and hosted/image acceptance gates.

- Before implementation, `cargo test -p lapis-app-core --test content_grants
--locked` failed to compile with one unresolved-import diagnostic for the
  missing registry and budget constant. An earlier draft also had a test helper
  shadowing error; that was corrected before the missing-feature baseline.
- The focused integration target covers credential secrecy/scope, instance
  isolation, malformed-input refusal, capacity, shutdown/drop retirement,
  cross-grant cursor isolation and closed-vault refusal.
- `cargo test -p lapis-app-core --lib services::content::grants::tests --locked`
  covers native-lock revocation while another grant remains usable, expiry
  cleanup and shutdown fencing all waiters before draining any grant.

The focused integration target passed 5/5; the concurrent lifecycle target
passed 3/3. `cargo test --workspace --locked` passed 725 tests, with 0 failures
and 7 pre-existing model/helper ignores. No new ignores or assertion waivers
were introduced. Strict workspace Clippy, protocol drift/TypeScript clients,
configuration, specification, formatting and edited-file Rustfmt all passed.
The new direct getrandom/subtle dependencies reuse the exact versions already
locked for the server; no existing dependency version or Git pin changed.

## Remaining boundaries

The registry is ephemeral and intended for controlled native integration tests.
Hosts must authorize issuance, schedule expiry cleanup and call blocking drain
outside async executors. Persistent grants, rotation, consent, OS custody, OAuth,
HTTP/CLI/MCP adapters and Desktop intake remain in the content backlog. Existing
model and hosted teardown qualification gaps remain unchanged.
