# Protected native Sync account state

15 November 2026. Owning Server change `rpupuuty`, SRV-SYNC-017.
Status: portable source qualified; OS custody and application
integration remain open.

## Behavior and scope

The account transport previously had no admitted protected-state repository.
The new native-only store requires an explicitly selected host custody port.
Profile-private metadata contains only logical/physical scope and stable public
client/binding identifiers. Profile and vault directory capabilities, server
origin and physical owner-lock identity bind the protected envelope. One OS file
owner retains the scope; replaced roots, metadata or locks fence access.
Secrets use bounded zeroizing buffers and are absent from profile/vault files.

A protected write retains its exact bytes and previous-value digest before
calling custody. An uncertain result blocks reads, new writes and close. Retry
inspects custody first: the desired value completes without another mutation,
the original value permits identical-byte retry, and unexpected state refuses
without overwriting evidence. Missing protected data never resets identity.
The host must retain the owner after a refused close.

The existing private-directory admission helper now uses the admitted profile
capability throughout traversal, compares physical identity and checks roots
before and after opening. All four existing Sync repository callers use this
owning helper directly. There is no compatibility re-export or fallback.

## Reproduction and focused checks

- Initial exact protected-state test was unavailable: unresolved protected module.
- First implemented exact run stopped at a locked dependency-feature update.
  Enabling zeroize serde support required a lockfile dependency edge; the pinned
  zeroize version remains 1.9.1. The exact persistence target then passed 1/1.
- Initial family passed 7/7. Expanding the shared admission helper first failed
  compilation for one missing `invalid` import; after repair, expanded family
  passed 8/8, including a fresh child process checking exclusive ownership and
  stable public identity after close. The child uses a RAM fake with no secrets.
- Six affected Sync integration suites initially passed 48/48 with no ignores. Strict
  Core all-target Clippy passed before the final additional recovery scenario.
- `pending_retry_refuses_foreign_custody_without_overwriting_evidence` passed
  1/1: no custody mutation after observing foreign bytes, pending ownership
  retained, then identical retry after the original value is restored.
- Initial spec check found a verification row separated from its table; the
  next found the missing mapped governance chapter. Both are repaired; complete
  specification, mdBook, spec-first and App ownership gates then passed.
- Doctor passed ten checks with one existing script-alias warning. QMD search
  for SRV-SYNC-017 passed after removing an extra unsupported `--` argument.
  Registry latest spec-validator is 0.2.0, already selected by this repository.

Logs are retained under `.local/sync-account-protected-*`. The initial global
Rust formatting check found baseline formatting differences; formatter-only
changes outside this slice were restored. Changed Rust files are checked
separately without rewriting unrelated source.

## Additional integrity regression and broad-run evidence

Review found that removing both binding metadata and custody could initialize
an existing scope again. The exact regression
`missing_binding_and_custody_do_not_reinitialize_an_existing_scope` failed
0 passed/1 failed before repair, then passed 1/1. A retained ownership file now
requires recovery when neither metadata nor custody can restore the original
identity. The exact assertion proves no replacement binding or custody write.
The atomicity and acceptance-count checks rejected an initial combined-sentence
bullet and then five bullets; the same constraints now use four atomic bullets.
Final six-suite coverage passes 50/50 with no failures or ignores, including
10 protected-state tests. Logs end in `missing-binding-{before,after}.log` and
`all-sync-final.log`.

The first workspace rebuild exhausted disk while linking, before test execution.
The protocol export run was interrupted while waiting for the Cargo lock.
After stopping task builds, obsolete task-only test executables were retired;
inventories remain in `retired-{baseline,intermediate}-tests.json`. The next
workspace compilation was deliberately stopped to repair the newly discovered
identity-loss regression. Neither incomplete run counts as qualification.
The fresh final workspace and strict Clippy results are recorded below. Unrelated source, caches and test receipts were preserved.

## Qualification limits

Tests inject an explicit RAM custody fake. They qualify portable source and
actual file-owner exclusion on macOS ARM64, not Keychain, locked OS credentials,
Tauri IPC, application lifecycle or any installed consumer. No account bearer,
proof or wrapping key crosses renderer IPC. Complete protected account
orchestration, endpoint enrollment, encrypted transfer and full Sync journeys
remain open. No package, image, production or installed application is changed.
Visual review remains separate; no baselines were updated.

## Final source qualification

`cargo +1.99.0 test --workspace --locked -j 2` passed 872 tests, zero failures
and seven existing non-Sync ignores across 175 result summaries, exit 0.
`cargo +1.99.0 clippy --workspace --all-targets --locked -j 2 -- -D warnings`
passed, exit 0, after the tests completed. Logs end in `workspace-final.log`
and `workspace-clippy.log`. The macOS test linker emitted its existing large
unwind-table warning; this is distinct from Rust lint qualification.

The initial global formatter exposed 35 unrelated baseline Rust formatting
changes; all were restored. Direct `rustfmt --check --edition 2024` on changed
Rust paths passes. Prettier, the canonical three-lane specification/App audit
and the final protocol-binding/type check pass. The interrupted protocol check
remains retained separately from the final successful run. No renderer, browser
or package UI behavior changed in this source slice.
