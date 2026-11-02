# Backup shutdown stage diagnostics

Date: 2026-11-03. Owning source extends lapis-server
`3f79b7c6dc883929ad7810a61e5680a07d33ae0e`, consumed by Desktop PR #10.
This is diagnostic observability, not a root-cause repair.

## Demonstrated boundary

Desktop workflow `41630554968`, Linux x64 job `125900941173`, ran at Desktop
source `c0e621244be61b01e5cc8b1c2bc93e0f55a36521` with
`LAPIS_APP_BACKUP_DIAGNOSTICS=1`. Both concurrent initialization attempts
reported success through `HistoryIndexing`. The unchanged
`original_and_managed_reads_share_one_client_without_initializing_original_history`
case then failed at its explicit `stop_backup` call, reported at
`src-tauri/tests/repository_original.rs:76`, with the sanitized public error.
The initialization evidence therefore does not support treating this attempt as
an initialization failure.

The same exact consumer case passed 1/1 locally on Linux and its scoped
`repository_original` group passed 4/4 with initialization diagnostics. Those
are non-reproductions, not retries that qualify the failed hosted attempt, and
they do not qualify the ARM64 boundary.

## Contract and diagnostic review

SRV-SRV-004 is unchanged. When the existing debug-only
`LAPIS_APP_BACKUP_DIAGNOSTICS=1` opt-in is selected, explicit stop now records
fixed stages from client admission and queue reservation through final capture,
watcher release and queue completion. A native final-capture failure is reduced
to one fixed category: busy, binding, IO, JJ, worker or other. The raw native
error is never formatted. Records contain only a process ID, attempt ID, fixed
stage/category, outcome and elapsed time; they exclude paths, credentials, user
content and vault data. Diagnostic output failure cannot fail accepted backup
work, release builds emit none, and the public error, ownership, ordering and
result are unchanged.

No automatic retry, quarantine change, timeout change or assertion change is
included. The original ARM64 quarantine remains until the original exact and
scoped cases demonstrate the cause and pass with the repaired owning source.

## Executed source validation

Runtime: local Linux x86_64, Rust 1.99.0, locked Cargo inputs.

```sh
rustfmt --edition 2024 --check \
  crates/lapis-app-core/src/services/backup/commands.rs \
  crates/lapis-app-core/src/services/backup/initialization.rs
cargo test -p lapis-app-core --lib services::backup::initialization::tests --locked
LAPIS_APP_BACKUP_DIAGNOSTICS=1 \
  cargo test -p lapis-app-core --lib services::backup:: --locked -- --nocapture
cargo clippy -p lapis-app-core --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

- Exact diagnostic tests: 4 passed, 0 failed, 0 ignored, 133 filtered out.
- Complete affected Core backup library family: 25 passed, 0 failed,
  0 ignored, 112 filtered out. Successful stop executions emitted
  `FinalCapture`, `ReleaseWatcher` and `QueueCompletion` stages.
- Strict Core all-target Clippy passed.
- The complete local Cargo workspace suite passed. Its pre-existing explicit
  ignores remained limited to isolated subprocess helpers and qualified ONNX
  asset cases; no ignored case was enabled, disabled or used as qualification.
- Both changed Rust files passed the scoped Rust 1.99 formatter check. The
  repository-wide formatter check exposed unrelated Rust 1.99 style changes in
  untouched files, so none of those files were rewritten.

## Outstanding qualification

Adopt this owning-source revision in Desktop, then execute the unchanged
Original/Managed case alone and in its integration group on macOS ARM64 with
diagnostics enabled. Preserve the first failing stop stage/category and do not
infer a fix from Linux success. A demonstrated failure category must be traced
to its owning implementation before any repair; only the original affected
platform acceptance can support quarantine removal.
