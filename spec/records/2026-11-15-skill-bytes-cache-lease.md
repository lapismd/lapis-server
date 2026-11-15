# Skill byte protection and cache lease release

## Native byte preservation

The exact `binary_local_changes_skip_the_entire_skill` regression failed once
because a binary local file was read as UTF-8. Snapshots now hash raw bytes;
edited binary references and added binary assets skip the complete skill and
retain its bytes. The exact regression passes.

## Cache lease regression

The first aggregate after that fix reported 150 library passes, one failure and
one existing ignored test. The original competing-installers scenario failed
at immediate lease reacquisition after joined installer threads. Its isolated
rerun passed, which alone is not qualification of the race.

A deterministic duplicate-descriptor regression reproduces the failure: closing
the owning descriptor leaves the OS lock held by its duplicate. A concurrently
spawning process can inherit that same open-file description. The cache lease
now explicitly unlocks on owner drop. No deadline or assertion is loosened.
The new exact case fails before and passes after; all eight cache tests pass.

## Aggregate evidence

`cargo test --locked -p lapis-app-core -p lapis-app-protocol -p lapis-cli`
passes 651 tests, zero failures and seven existing ignored cases. Strict
Clippy for those packages and all targets passes. Focused files are formatted.
The attempted workspace-wide rustfmt check also identifies existing formatting
in unrelated backup, projection, protocol and server files; those source files
are retained unchanged. No new test quarantine or audit exception is introduced.

Source qualification does not qualify a hosted image or Desktop adoption.
Registry App Profile/API intake and host acceptance remain separate gates.
