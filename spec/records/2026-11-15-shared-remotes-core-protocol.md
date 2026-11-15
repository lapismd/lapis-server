# Shared repository selection and remotes Protocol source

Source change `ksywutul` builds on encrypted custody change `swtrxykt`.
SRV-SRV-004 retains shared native policy ownership; SRV-SRV-015/016 custody
and authenticated host adoption remain separate gates.

Core now owns the extracted Desktop version-one backup selection policy through
`repository::backup_selection`. It preserves configuration revisions, fetched
anchors, private connection files and native-derived pending push receipts.
The generic configure input contains no token. Explicit saving performs no
network operation, and an active push prevents rebinding/disconnection.
Desktop adoption will remove its maintained old implementation.

The extraction exposed revision exhaustion: disconnect could publish a value
that its next read rejects. The exact new test
`cargo test -p lapis-app-core --lib repository::backup_selection::tests::exhausted_revision_does_not_publish_an_unreadable_configuration --locked`
failed once. The corrected policy refuses the update before changing bytes;
all four selection cases pass afterward. The initial absence probe overlapped
compilation with edits and is not claimed as a pre-change test result.

Protocol generates path-free target/revision commands, capabilities, opaque
credential inventory and push review/status types. Push accepts one native
review rather than arbitrary commit/destination, force or all-bookmark fields.
Secret-bearing Rust inputs deliberately implement neither Debug nor Serialize.
An exact wire/declaration test initially found one generated field/tag mismatch
(2 passed, 1 failed); explicit ts-rs annotations repair it. All three exact
Protocol cases now pass, including additional-field rejection and secret-free
inventory.

Validation uses Rust 1.99.0, Node 24.10.0, pnpm 10.34.5, private target
`.local/remotes-target`, two Cargo jobs, no incremental/debug information:

- Complete `cargo test -p lapis-app-core -p lapis-app-protocol --locked`:
  622 passed, zero failed, 7 existing opt-in ignores across 123 suites.
- All-target strict Clippy for both crates: pass. Touched Rust files are formatted
  independently; unrelated existing formatter changes were restored.
- `pnpm protocol:generate` and `pnpm protocol:check`: generated schema/declarations
  match Rust and the TypeScript consumer compilation passes.
- Root `pnpm check`: zero Svelte errors/warnings; App ownership passes.
- Root `pnpm test`: 132 cases across 31 files pass, zero failures.
- `pnpm build` and `cargo build -p lapis-server --locked`: pass. Existing Web
  chunk-size and macOS large `__eh_frame` linker warnings remain explicit.
- `pnpm spec:check`: canonical validation, mdBook, mapped spec-first and App
  ownership pass.

This qualifies shared source policy and generated contracts. Actual shared
orchestration, both target transports, credential/forge adapters, authenticated
HTTP/Tauri admission, settings registration and cold native/hosted acceptance
remain outstanding. No public crate was published and no host gains remote
mutation authority from this receipt.
