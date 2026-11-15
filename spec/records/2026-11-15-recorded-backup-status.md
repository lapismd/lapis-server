# Recorded backup working-copy status

The approved footer needs native status even when a vault has no original JJ
repository. SRV-SRV-004 now selects an existing managed backup in that case and
reports its latest recorded checkpoint and native parent-diff statistics. An
existing original retains its bounded physical observation. An invalid original
refuses instead of silently changing sources.

The backup path reuses `HistoryReader` and the existing operation-pinned native
commit-summary and file-diff requests. It aggregates the native exact line
hunks; no scanner or diff algorithm is copied. Binary/structural/complex content
retains unknown line totals and an uncounted-file count. Complete native query
limits refuse rather than returning partial statistics. Before/after operation
heads, source authorization and final source selection are checked before the
frame returns. Reads neither initialize JJ nor start or acquire capture.

## Regression evidence

`node .local/remotes-validation.mjs test --locked -p lapis-app-core --test
repository_working_copy` initially reports two passes and one failure:
backup-only status is Unsupported. The exact target after implementation passes
three tests. Strengthening the invalid-original case produces four final passes,
zero failures/ignores.

The backup regression runs text and binary variants against real native saved
checkpoints while a Session retains its capture lease. Later disk modifications
and an additional unrecorded file remain unchanged and absent from the reported
checkpoint. Actual native diff totals match the recorded changes. Operation and
latest checkpoint remain unchanged, no `.jj` appears in the vault, and detached
clients refuse. Existing original physical counts and uninitialized-vault refusal
also pass. A deliberately invalid original retains its marker and refuses even
when a valid backup exists.

The entire Core suite passes: 605 passed, zero failed and nine existing opt-in
ignores across 109 suites. Strict all-target Clippy with `-D warnings` and the
Core build pass. The preceding native-provider workspace evidence remains
separate; this later qualification covers the working-copy extension.

Validation uses immutable tracked archives, Node 24.10.0, Rust 1.99.0, the
existing isolated cache `604a978da80314ea5eba`, and remote image
`sha256:6031c86d7c26d9e7011c958374462c600b10aa6a586ad7196d741756df0a5a32`.
`pnpm spec:check`, the App-ownership audit and `pnpm format:check` pass.
Changed Rust source passes Rustfmt in edition 2024.

## Remaining gates

The version-one frame already supports source `backup`; no Protocol schema or
renderer secret field changes. A missing recorded checkpoint and unsupported
multi-parent/complex observations remain unavailable. This status describes the
saved checkpoint, not later uncaptured disk bytes. Both actual application
compositions and cold host acceptance remain required before the feature release.
