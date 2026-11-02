# Backup initialization stage diagnostics

Date: 2026-11-02. Owning source starts at lapis-server
`4150cac6f2fef28b128421e4e3a851b848acdcf3`, consumed by Desktop PR #9.
This is a diagnostic observability change, not a root-cause repair.

## Contract and acceptance review

SRV-SRV-004 is unchanged: Core retains vault admission, accepted work, backup
storage, capture ordering and native ownership. No Cargo source, dependency,
feature default, public error, quarantine, assertion or timeout changes.
The debug-only `LAPIS_APP_BACKUP_DIAGNOSTICS=1` opt-in records a bounded set of
fixed initialization stages from admission/queue reservation to first capture.
Records include safe categories, process/attempt IDs, elapsed time and OS codes
where error types retain them. They never format arbitrary native error text,
paths, credentials or vault content; a stderr failure cannot fail backup work.
Native repository String errors no longer retain typed OS codes, so their
records deliberately report the native category without parsing error strings.
Release builds do not emit diagnostics.

The original intermittent ARM64 failure is unresolved. Linux success is
non-reproduction, not proof of a fix. The previously recorded two-vault backup
History binding-validation failure and unchanged isolated rerun remain in the
[projection intake receipt](2026-10-30-native-projection-dependencies.md).
No historical receipt was edited and no quarantine was removed.

## Executed source validation

Runtime: Linux x86_64, Rust 1.99.0 (`b940084d7`, 2026-09-28), checksum-verified
Jujutsu 0.45.1, locked Git/crate inputs, three build workers. Commands ran from
the owning checkout with workspace-local Rust tool directories:

```sh
export CARGO_HOME=/workspace/.tools/cargo
export RUSTUP_HOME=/workspace/.tools/rustup
export PATH="$CARGO_HOME/bin:/workspace/.tools/bin:$PATH"
export CARGO_BUILD_JOBS=3
cargo test -p lapis-app-core --lib services::backup::initialization::tests --locked
LAPIS_APP_BACKUP_DIAGNOSTICS=1 cargo test -p lapis-app-core --lib services::backup:: --locked -- --nocapture
cargo clippy -p lapis-app-core --lib --locked -- -D warnings
cargo clippy -p lapis-app-core --all-targets --locked -- -D warnings
```

- Exact diagnostic tests: 3 passed, 0 failed, 0 ignored, 133 filtered out.
  Cold source/dependency compile took 8m34s; tests completed in 0.00s.
- Complete affected Core backup library family: 24 passed, 0 failed,
  0 ignored, 112 filtered out; 3.44s. These include the three diagnostic tests,
  real backup initialization and accepted-work cancellation/shutdown drain.
  Fixed stage records emitted during this run. This is one Linux execution,
  not a hosted ARM64 stability sample.
- Strict Core library Clippy passed in 1m40s including the dependency check
  graph. Strict Core all-target Clippy passed in 12.08s with cached inputs.
- Changed Rust files passed rustfmt; `git diff --check` passed.
- There were no failed test attempts in this implementation run. Timing is
  local source validation, not an ordinary-PR feedback benchmark or a hosted
  cache cold/warm comparison.

## Outstanding qualification

Publish this owning-source change on a review branch before updating Desktop's
exact Core pin. Do not replace it with a consumer alias or external Cargo path.
Run Desktop's exact Original/Managed reader case and its narrowly scoped group
on ARM64 against the adopted source, retaining the first failing stage and
original independent-history/source-byte/start-stop assertions. Keep the
quarantine until a demonstrated cause is fixed and the original acceptance
passes. Full server workspace/hosted CI, original Desktop native acceptance,
and release/distribution qualification remain pending; these focused checks do
not promote those gates.

Specification validation's first complete check passed its seven validators and
mdBook build but failed spec-first because the catch-all mapping also requires
the governance chapter. Added its unchanged-contract confirmation, preserving
that failed check; this is a specification review correction, not a failed or
retried backup test.
The corrected `spec-validator check --json` passed all three lanes: seven
validators, mdBook build and spec-first (both mapped canonical chapters).
