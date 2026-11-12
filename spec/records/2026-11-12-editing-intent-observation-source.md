# Native editing-intent invalidation source

The user approved SRV-HST-023 and the optional Notes API observation port.
This record qualifies the native service and authenticated HTTP boundary.
The packaged browser still uses the released API without the observation port;
frontend wiring, API registry intake and public-image acceptance remain pending.
No production deployment or user-vault mutation is included.

## Behavior and original regression

The exact command `cargo test --locked -p lapis-server --test editing_intent_stream`
failed its single authorized registration case with HTTP 404 before implementation.
That unchanged target passed after implementation. Three expanded HTTP cases now
cover live metadata invalidation, authentication/scope rejection and idle browser
session revocation. Frames contain only a revision, without editor IDs, paths,
drafts or file bytes. Explicit recover/discard preserves saved bytes.

Core publishes coalescible latest-revision invalidations under its existing vault
state lock when managed editor metadata changes. Exact retries, List and Inspect
do not invalidate. Detach, captured recovery/discard and legacy managed dirty,
saved and clean retirement paths retain coverage. Registration is atomic with
state changes; a slow observer retains the latest revision rather than an
unbounded event history. Registration and reconnect require authoritative List.

The HTTP adapter retains the existing shared 64-reader bound, 30-second response
lease and continuous authentication, admitted vault/generation, client and
physical-root checks. Keep-alive comments do not invalidate claims. Observation
holds no work lease and creates no persistence owner or filesystem watcher.

## Executed qualification

- Three new native observation cases and the existing 17 intent/recovery cases
  pass, including 1,000 coalesced transitions, foreign vaults and replaced roots.
- Three new HTTP observation cases, four existing file streams and the existing
  editor-state write case pass.
- Full Linux `cargo clippy --workspace --all-targets --locked -- -D warnings`
  and `cargo test --workspace --locked` pass: 799 tests, zero failures and the
  same seven existing opt-ins ignored.
- The complete unchanged macOS HTTP family passes all 54 tests. An earlier
  broader run had 53 passes and a connection reset in the oversized-body case;
  that exact case subsequently passes alone, followed by the complete 54-case
  family. No assertion, limit or deadline changed.

Local full-workspace Rust attempts exhausted the shared Mac disk while linking.
Those are interrupted qualification, not passes. Only regenerable outputs in
this checkout's `.local/rust-core-target` were cleaned; source, receipts, user
data and unrelated Docker resources were preserved. Complete qualification then
ran in task-owned source/Cargo volumes on Docker context `omarchy`, using image
`sha256:dc08a890565545fd5e1d99b5ca57b70dc7dc842c3af38e07bbc684fd4c4b751a`,
Rust 1.99.0 and upstream JJ 0.45.1. The successful worker and both owned volumes
were removed. Source archive SHA256:
`a46b8d896f5b8dc4e5f67290b4f318cba76908839536043fefd9d15cbe13e93a`.

Native source revision is `246fda041a9fd30292de3066bba1e86816d6b74c`; subsequent
documentation updates do not alter its Rust tree. Receipts and first failures
remain under ignored `.local/editing-intent-observation/`, including
`remote-native.json`, `remote-native.log` and `http-complete-after-isolation.log`.
Canonical validation, mdBook, spec-first, App ownership, changed-file Rustfmt
and Prettier pass. Initial acceptance-list shape/count and governance mapping
diagnostics were corrected without changing runtime assertions or policy.
This is native source evidence, without a browser-idle or released-image claim.
