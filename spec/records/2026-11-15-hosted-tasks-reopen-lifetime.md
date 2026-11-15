# Hosted Tasks fixture reopen lifetime

15 November 2026. SRV-GOV-002 and SRV-HST-021. Source qualified; final-head hosted CI remains required.

Server85 head0000a320, run43758498297, job132947767687 failed its actual
Notes UI/catalog lane:14Tasks cases passed and1failed. Concurrent Tasks recovery
reported an aborted `.lapis/app.json` configuration fetch from the departing
App at teardown. Existing runtime-error assertions remain unchanged. Raw hosted
log is retained under .local/sync-protected-hosted-catalog-failed.log.

The unchanged original exact case passed locally1/1, so that warm run does not
explain or waive the failure. A stronger original boundary now observes successful
DELETE of the original native client before fresh client POST admission on the
same page. On an unchanged fixture, a fresh renderer build and native main build
reproduce0/1: only fresh admission occurs, with no prior native release. The
source fixture navigated directly while a ready App still owned observers and
configuration IO. Its existing restart/stop helpers already awaited guarded leave.

The fixture now crosses the real guarded logout boundary before reopening a
ready App. Concurrent recovery uses independently authenticated browser contexts
so one logout cannot revoke the other App while it still owns observers. Its regression keeps native Tasks unknown-outcome/reference assertions and
zero runtime/plugin errors, and adds release-before-admission ordering. No error
is filtered, timeout raised, assertion loosened or case quarantined. Reopens
still use a fresh browser document and a fresh native admission.

Before logs: .local/sync-protected-catalog-exact-before.log and
.local/sync-hosted-leave-before.log. The failed deterministic trace/context is
preserved in .local/sync-hosted-leave-before-artifacts before rerunning. Fresh
registry-frozen installation, renderer and native build logs share
.local/sync-hosted-leave-\*. The shared fixture affects15Tasks and2Properties
cases; qualify all17after the exact regression, then owning check/test/build,
Storybook interaction/accessibility and static lanes. No visual baseline change.

The first repair attempt used Manage Vaults alone; the strengthened case still
failed0/1 (`after.log`). Trace inspection proves this chooser intentionally
retains its App for Return to workspace, so it is not a teardown boundary.
The final fixture uses the existing guarded logout helper, and the concurrent
Apps each own an independent authenticated browser context. Both leave through
that helper before destroying their documents. The failed attempt and trace
remain in .local/sync-hosted-leave-manage-only-artifacts; no production behavior
or authentication contract is changed.

The complete affected browser cohort passes15Tasks and2Properties cases with
zero skips/failures, including the strengthened concurrent recovery case.
The owning JavaScript lane passes4CLI distribution and138unit cases; types and
App ownership pass. All64Storybook interaction/accessibility cases pass. Fresh
renderer production build passes with existing chunk-size advisories.

The first static Storybook build failed ENOSPC after transformation; this is a
retained infrastructure failure, not passing static acceptance. With no Cargo,
rustc, linker or task integration test process active,12completed metadata-matched
Mach-O integration executables in the task-owned Server target were retired,
reclaiming3,099,446,688bytes. Paths, metadata, sizes and pre-retirementSHA256are
retained in .local/sync-hosted-leave-retired-tests.json. Source, dependency
metadata, running-server binary, fixtures, reports and unrelated caches remain.
The unchanged static build, static-browser acceptance and canonical checks
must pass before this slice is committed. Logs retain the first failure and
explicit build-recheck separately. No memory/size threshold was changed.

## Final qualification

- Strengthened actual client-lifetime regression:0/1before;1/1after, and final cold-process recheck1/1 after guaranteed cleanup changes (`exact-final.log`).
- Complete shared-fixture browser cohort:15Tasks/15passed and2Properties/2passed, zero skips (`tasks.log`, `properties.log`).
- `pnpm check`:types0errors/0warnings and App-ownership audit pass (`check.log`).
- `pnpm test`:4CLI-distribution and138unit tests/32files pass, zero failures or skips (`test.log`).
- `pnpm build`:fresh renderer production build passes (`build.log`).
- `pnpm test:storybook`:64cases/6files pass with interaction/accessibility checks (`storybook.log`).
- `pnpm build-storybook`:unchanged build passes after bounded task-only retirement (`storybook-build-recheck.log`).
- `pnpm test:storybook:static`:5/5real static-browser cases pass, zero skips (`storybook-static.log`).
- `pnpm spec:check`:validation,mdBook,spec-first and App-ownership pass (`spec.log`).

Logs above share .local/sync-hosted-leave-. Source uses task Node24.15.0,
pnpm10.34.5 and Rust1.99.0 on macOS ARM64. Frozen registry manifests/lock and the
existing documented private prerequisite cohort remain unchanged. This is actual
task-owned Rust HTTP/browser acceptance; it does not qualify Desktop Keychain,
Tauri, installed applications or complete Sync. No production profile or OS
credential is used. UI/package warnings and the native large-unwind-table linker
advisory remain visible. No screenshot or visual baseline was updated.

The final regression asserts native release precedes fresh admission and keeps
one accepted Task submission, no blind retry, retained references across another
App's saved settings, recovered task bytes and zero runtime/plugin errors. Each
browser App has its own authenticated context and leaves through its guarded
logout before destroying its frame. Finally blocks guarantee task-owned context
and server cleanup even if a departure fails. Final changed-file formatting,
types and canonical metadata checks run before the unsigned commit; hosted CI
must qualify the submitted head before merge.
