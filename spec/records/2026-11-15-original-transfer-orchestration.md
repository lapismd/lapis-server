# Original native transfer orchestration

Core now implements manually admitted original test/fetch/review/push/observe/stop
through the generated path-free commands. `RemoteStorage::with_transfers(true)`
is a native host decision; the default remains inventory/save-only. A backend
credential resolver returns native Git credentials only after scoped inventory
admission. No stored secret crosses into the renderer. Backup transfer flags,
credential mutation/sign-in, scheduling and host adoption remain separate work.

Each accepted transfer holds the private selection process lock across source,
configuration, credential and native operation admission. Invalid review and
request identities refuse before resolving credentials. The native original
writer is constructed only for explicit accepted commands. Save never transfers;
test observes reachability/authentication without fetching or publishing. Fetch
uses JJ tracking semantics and retains real counts/time without snapshotting,
checkout writes or changes to a client-owned history pin.

Native immutable reviews persist in a separate source/connection-bound private
journal. Execution durably consumes a review before native revalidation/reserve;
a consumed review cannot replay. Native pending receipts remain authoritative
across detach/restart. New publication/fetch/rebinding refuses while an earlier
publication needs observation or explicit local stop. Stop performs no network
work and needs no credential. Missing binding metadata cannot hide an active
receipt or authorize connection replacement.

## Before and after

- `repository_remotes_transfers::unconfigured_transfer_refuses_before_creating_transport`
  first failed once: Unsupported instead of an actionable missing-connection
  refusal. The exact case now passes.
- The first real SSH Core run failed once because immutable working-copy status
  became unavailable after a metadata-only fetch. This was reproduced/fixed in
  the owning jj-store reader, separately qualified by E-ORIGINAL-REMOTE-005 and
  PR #83 at `a1d5efa0d96024075e3971f278db76684c49cb18`. The Core exact transport
  scenario then passed, retaining the original assertions.
- A strengthened real uncertain-outcome case failed once when a missing
  `vault.json` hid its active receipt. Core now delegates absence/corruption
  handling to native pending-publication lookup for both guarded saves and
  transfer projection. The exact case passes without weakening recovery checks.
- All 17 focused selection/runtime/working-copy/transfer cases pass. Token
  preflight and a paused resolver prove zero resolution for invalid identities,
  cross-process rebind refusal, unchanged selection bytes and accepted-work
  survival after waiter cancellation/detach. Denied resolution initializes no
  transport and performs no network access.
- The real SSH scenario passes with exact one-bookmark publication, persistent
  review after restart, unsnapshotted-file exclusion, unchanged checkout bytes,
  real fetched counts, status after fetch, consumed-review refusal and
  lost-response restart recovery with no additional receive. Removing/restoring
  the native binding record additionally proves fail-closed recovery.
- An initial broad run passed 802 tests, zero failures and eight opt-in ignores.
  After the recovery refinement, the final complete Core/Protocol/Server run
  again passes 802 tests, zero failures and eight opt-in ignores in 157 suites.
  The real SSH case is one of those opt-ins and is qualified separately above.
- Strict all-target Clippy passes for Core, Protocol and Server. Configure,
  generated Protocol checks, canonical specification/App ownership and complete
  Prettier checks pass. Changed Rust files pass direct Rustfmt; the inherited
  unchanged-file whole-workspace drift remains in the earlier receipt.

## Commands and runtime

Native validation uses exact tracked JJ archives, 2 CPUs/8 GiB, Rust 1.99 and
image `sha256:6031c86d7c26d9e7011c958374462c600b10aa6a586ad7196d741756df0a5a32`.
The disposable SSH runner explicitly uses Node 24.10.0 and is adapted with
provenance from jj-store's qualified harness. Keys, agent, HOME, strict known-host
files, bare Git endpoint and response-loss fault flags are disposable; host
trust and user profiles are untouched. No local source alias or package patch
is used. Cargo locking changed only the 18 jj-store source coordinates.

- `node .local/remotes-validation.mjs test --locked -p lapis-app-core --test
repository_remotes_transfers --test repository_remotes_runtime --test
repository_remotes --test repository_working_copy`
- `node .local/remotes-validation.mjs --ssh` launches
  `node scripts/test-repository-remotes-ssh.mjs` inside that isolated worker.
- `node .local/remotes-validation.mjs test --locked -p lapis-app-core -p
lapis-app-protocol -p lapis-server`
- `node .local/remotes-validation.mjs clippy --locked -p lapis-app-core -p
lapis-app-protocol -p lapis-server --all-targets -- -D warnings`
- `node .local/remotes-validation.mjs build --locked -p lapis-server`
- Node 24 `pnpm configure`, `pnpm protocol:check`, `pnpm spec:check`,
  `pnpm format:check`; direct Rustfmt on all changed Rust files.

## Intake and remaining gates

The new SDK pin is temporarily qualified under PR #83; its required hosted CI
and merge must complete before permanent delivery. Shared backup orchestration,
credential-provider startup/mutation/sign-in, HTTP/Tauri adapters, production UI
adoption, automatic synchronization and both cold application acceptances remain
open. This source receipt does not qualify any installed/deployed host or a
whole-feature release.
