# Native account leave preparation

15 November 2026. SRV-SYNC-020. Native source qualified; Tauri/OS integration remains open.

A host must be able to drain account work before checking other native owners
without closing custody when leave is refused. The exact
`refused_host_leave_drops_preparation_and_retains_the_original_account_owner`
availability probe fails compilation with four missing prepare_close method
errors in its new target; no case qualifies before implementation. Log
.local/sync-close-preparation-before.log.

Core adds an original-scope preparation guard: one native fence, asynchronous
permit wait, blocking settled-custody/root preflight, and explicit blocking
commit after the host approves leave. Dropping the guard releases the fence
without releasing the original session. Pending custody refuses preparation;
failed preflight replaces prior Ready observations with RecoveryRequired.
A host owns the guard across its native lifecycle and commits off async/UI
executors. No guard enters renderer state and no second scheduler is introduced.

The full original account close/drain semantics remain separate and unchanged.
Source qualification uses explicit fixture custody, never OS credentials.
Tauri/App integration, actual OS qualification and all later Sync work remain
open. Logs share .local/sync-close-preparation-\*.

## Final source qualification

- Exact missing-method availability boundary: four compiler errors before implementation; repaired original regression 1/1, zero ignores (`after.log`).
- Complete preparation target:4passed/0failed/0ignored (`family.log`), including original-owner retention after refused leave, detached work with one blocking worker, actual cold-process ownership exclusion and recovery after uncertain custody.
- Affected Core account targets:36passed/0failed/0ignored across six targets (`account.log`).
- Actual current-server account sessions:6passed/0failed/0ignored, including cold-process restoration (`server.log`).
- `cargo test --locked --workspace`:892passed/0failed/7existing non-Sync ignores across180result summaries, exit0 (`workspace.log`).
- Strict all-target workspace Clippy, protocol Rust/TypeScript parity, canonical validation/mdBook/spec-first and App-ownership checks pass (`clippy.log`, `protocol.log`, `spec.log`).

Log suffixes above share `.local/sync-close-preparation-`. Rust commands use
Rust1.99.0/macOS ARM64 and the task-owned `.local/rust-core-target`; fixture
custody does not qualify Keychain. Changed Rust files pass Rust2024 format
checks. No ignore, assertion or protocol was weakened. Existing linker
large-unwind-table advisories remain visible. Final metadata-only canonical
checks are retained in `spec-final.log` under the same prefix.

The host owns preparation across its other native leave checks. Dropping the
guard preserves the same account owner; committing releases only that original
scope after draining admitted work. This qualifies Core source behavior only.
Actual Tauri leave cancellation, OS custody and complete Sync remain open.
