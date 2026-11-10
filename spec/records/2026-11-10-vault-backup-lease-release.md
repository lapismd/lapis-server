# Vault backup command lease release

Date: 2026-11-10. This record adopts jj-store revision
`076e418cecbe204925aea874bef8aa8a680269a9` in Core after the owning source
reproduced and repaired a completed-command lease lifetime defect.

## Demonstrated boundary

Desktop's unchanged `repository_original` pair intermittently failed on Linux
after a completed backup `Session` or `Binding::relink` returned. With
`LAPIS_APP_BACKUP_DIAGNOSTICS=1`, Core reduced the native error to
`native-busy`; temporary owning-source instrumentation then showed that the
same binding's operating-system lock remained held after the Rust owner had
completed. The original pair failed locally within 3 to 49 repetitions before
the repair.

jj-store now owns an explicit command-lease guard. Dropping either a completed
session or relink command explicitly unlocks the file before closing it. The
existing non-blocking acquisition and `Busy` result for a genuinely overlapping
same-binding command remain unchanged. There is no retry, delay, deadline,
assertion, process-boundary or quarantine change.

## Owning-source evidence

- The new jj-store regression performs 512 alternating completed session and
  relink cycles and proves the immediately following command acquires the same
  binding.
- The complete affected `vault_backup` integration binary passed 6/6.
- Strict all-target `jj-store-jj` check and Clippy, Rust formatting,
  specification validation and spec-first validation passed.
- Desktop's original two-test pair passed 500 consecutive local Linux runs
  against the repaired owning source.

## Consumer qualification

Core pins the exact repaired revision in its manifests and lockfile and aligns
its direct `rustix` pins with the repaired source's exact 1.1.6 dependency. Its safe
debug diagnostics classify native failures without retaining native text,
paths, credentials or vault content; public errors and release behavior remain
unchanged. The affected Core backup family, strict all-target Clippy and the
Desktop original pair and scoped group must pass against this pin before this
record is complete.

The exact four diagnostic tests passed 4/4 and the complete affected Core
backup library family passed 25/25 locally on Linux against the pinned repair.
Strict workspace all-target Clippy passed with warnings denied, followed by the
complete locked workspace test suite. Its existing explicit ignores remained
the isolated process helpers and qualified ONNX asset cases; no ignore or
selection changed. The changed Rust source passed scoped Rust 1.99 formatting;
repository-wide formatting still reports unrelated existing Rust 1.99 drift in
untouched files, which this slice did not rewrite.

Linux proves the reproduced hosted boundary but cannot qualify the retained
macOS ARM64 quarantine. The original ARM64 case and scoped group must still run
with diagnostics enabled before that quarantine may change.
