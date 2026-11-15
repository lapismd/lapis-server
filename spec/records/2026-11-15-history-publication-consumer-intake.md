# Immutable history publication: Server consumer intake

15 November 2026. Owning change `uymowypk`, based directly on Server main
`0167f020`. Account transport and recovery are separate changes. Status:
local source/consumer qualification passed; upstream and hosted merge gates remain open. No activation.

## Problem and owning repair

The unchanged `http_files` case
`backup::hosted_profile_keeps_native_backup_after_detach_and_drains_final_saved_bytes`
failed inside `HistoryReader::open` with `Binding`. Its exact single-case run
passed 1/1, while the complete target reproduced 53 passed/1 failed twice.
Retained logs: `.local/sync-account-recovery-workspace.log` and
`.local/sync-account-recovery-http-files-rerun.log`. No assertion, retry,
selection, deadline or ignore changed in the consumer.

JJ Store's deterministic native regression holds a newly published operation
beside its ancestor: 0/1 before, 1/1 after. Immutable reads select the unique
maximal operation in memory without changing heads or unsnapshotted files.
Genuine divergence and traversal exhaustion still refuse. Owning source evidence
is `E-VAULT-HISTORY-FRONTIER-001` in upstream PR #86. Its rebase also preserves
#85's capture-independent publication review and evidence.

## Final dependency selection

All five direct Core/Server JJ dependencies, the manifest guard and canonical
SRV-SRV-002 select `87be3ad68530653c6c6fd1e5a2463a217cf3f044`. The lockfile
resolves all ten JJ Store crates consistently from that exact Git revision.
There is no path override, cache edit, copied reader, wrapper or crate tag.
The source requires `redb` 4.5.0 instead of 4.4.0. Both the previous accepted
Server lockfile and this candidate already select `jj-lib` 0.46.0; an earlier
task handoff confused that library with stock JJ CLI 0.45.1.

The prior accepted source was `076e418cecbe204925aea874bef8aa8a680269a9`.
The earlier candidate `c2f743c848775e5baf05577a5239488a806105bb` was qualified
with the account changes before upstream #86 needed another rebase. Evidence
for that earlier tree remains below and does not qualify the final pin.

## Final-pin consumer evidence

Commands use macOS ARM64, Rust 1.99.0 and task-owned
`.local/rust-core-target`. These are source/native HTTP consumer results.

- `cargo +1.99.0 test --locked -p lapis-server --test http_files backup::hosted_profile_keeps_native_backup_after_detach_and_drains_final_saved_bytes -- --exact`:
  1 passed/0 failed/0 ignored. Log `.local/sync-rebased-history-exact.log`.
- `cargo +1.99.0 test --locked -p lapis-server --test http_files`:
  54 passed/0 failed/0 ignored. Log `.local/sync-rebased-history-http-files.log`.
- `node scripts/configure.mjs`: pass, consistent source admission.
- `pnpm spec:check`: specification validation, mdBook, spec-first and
  App-ownership audit pass. Changed governed files pass Prettier.
- `cargo +1.99.0 test --workspace --locked -j 2`: 844 passed/0 failed/
  7 existing non-Sync ignores across 170 result summaries, exit 0. Log
  `.local/sync-rebased-history-workspace.log`.
- `cargo +1.99.0 clippy --workspace --all-targets --locked -j 2 -- -D warnings`:
  passed; log `.local/sync-rebased-history-clippy.log`.

`spec-validator doctor` passed 10 checks with one existing script-alias warning;
QMD native index opens and canonical SRV-SRV-002 search succeeds.

## Earlier candidate results and retained failure

At c2f743c8 with account transport/recovery, the exact backup case passed 1/1,
then unchanged `http_files` passed 54/54. Logs:
`.local/sync-history-intake-exact.log` and
`.local/sync-history-intake-http-files.log`. Strict all-target workspace Clippy
passed in `.local/sync-history-intake-clippy.log`.

The widened workspace run stopped at 64 result summaries: 393 passed, 1 failed,
6 existing ignores. The failing existing case was
`fifo_capacity_competing_clients_and_individual_cancellation_are_serialized`
at the eight-second queue completion deadline in
`crates/lapis-app-core/tests/support/native_queue/mod.rs:210`. Its log remains
`.local/sync-history-intake-workspace-resumed.log`. Clippy had overlapped the
early part of the run; contention is a hypothesis, not a proven root cause.

Without changes or build overlap, the exact queue target passed 1/1 in 6.57
seconds and its complete family passed 3/3 in 7.87 seconds. Logs:
`.local/sync-history-intake-native-queue-exact.log` and
`.local/sync-history-intake-native-queue-family-rerun.log`. The first family
command was queued while the exact case still compiled and deliberately stopped
before execution (exit 130); that log is also retained.

The complete locked workspace then passed alone: 862 passed/0 failed/7 existing
non-Sync ignores across 174 result summaries, exit 0. Log:
`.local/sync-history-intake-workspace-final.log`. No assertion, deadline, skip
or selection was weakened. These results apply to the earlier source/tree.

## Task-owned build resource maintenance

An initial widened build stopped deliberately with SIGINT (exit 130) at 650 MiB
free before tests executed; `.local/sync-history-intake-workspace.log` retains
it. With Cargo stopped, 103 superseded task-only test executables were retired
only where a newly built counterpart existed, reclaiming 14,723,010,560 bytes.
Inventory: `.local/sync-history-intake-retired-executables.json`.

Before the final-pin full rebuild, the exact test had finished and no Cargo
process remained. Retired 251 older task-only test executables identified from
the current workspace Cargo target inventory, reclaiming 17,225,766,832 bytes.
The new final-pin exact/family executable, all reports and source were retained.
Inventory: `.local/sync-rebased-history-retired-tests.json`. No unrelated
checkout, build root, credential, user vault or installed application changed.

## Remaining gates

The existing macOS large-unwind-table linker warning remains visible. This
receipt does not qualify Tauri IPC, Keychain custody, installed applications,
Linux images, independent-device Sync journeys or release. Upstream merge and
green hosted consumer checks are required before this intake merges. Final-pin local
workspace tests and Clippy pass; no hosted or platform gate is inferred.
