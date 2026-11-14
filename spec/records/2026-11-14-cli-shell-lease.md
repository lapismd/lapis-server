# CLI shell cleanup lease

A deterministic regression held the existing setup lock after removing the
command while retaining its marked shell block. PATH-only uninstall incorrectly
succeeded and removed the block: one failure before the fix. Uninstall now holds
the same per-user lease through command and shell cleanup, including the
PATH-only path. The regression verifies refusal leaves the shell bytes intact,
then cleanup succeeds after releasing the lock.

The exact regression, full installer/CLI suites, workspace Rust tests and strict
Clippy qualify this correction. Consumer CLI preparation must adopt the source
revision containing this fix; the earlier receipts remain immutable.

Results: the exact regression passes, all 9 installer and 25 complete CLI cases
pass, and the full Rust workspace passes 810 tests with 7 existing ignored.
Strict workspace/all-target Clippy passes. Specification validation initially
required the new acceptance details to use atomic bullets; the corrected source
contract retains the same locking behavior.

Formatting evidence is CLI-scoped: `rustfmt --edition 2024 --check` passes for
the changed installer and regression file. An exploratory workspace format
rewrote 35 unrelated existing files; those edits were restored byte-for-byte
before commit. This narrows the earlier receipt’s broad Rust-format wording.
Mapped governance/server chapters also retain the shared installer lease rule.
