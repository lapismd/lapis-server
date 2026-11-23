# CLI release version custody for Homebrew

The original exact regression, `node --test scripts/cli-version.test.mjs`, failed
one case because Cargo reported 0.1.0 while Changesets owned 2.9.1.
Version preparation now preflights and synchronizes the CLI manifest and lock
entry after Changesets applies the release plan. Archive jobs fail before the
build on any drift; normal check/spec lanes retain the same regression.

The intermediate malformed-lock test caught a missing write phase (one failure).
The corrected updater preflights both files before writing either one.

Validation: the exact regression now passes two cases. All 12 version,
distribution and release-workflow cases pass, as does the complete CLI Rust
suite. Canonical specification checks pass. Versions: Node 26.8.2, pnpm 10.34.5,
Rust 1.99.0, Changesets 3.0.3 and latest spec-validator 0.2.1. Doctor and QMD
semantic discovery passed; existing custom-alias warnings remain.

Published v2.9.1 archives are immutable and retain their original binary
identity. This repair has a new patch Changeset; the ensuing release supplies
the correct identity for Homebrew's executable-version acceptance. No existing
release, installed command, profile, or credential is replaced by source tests.
