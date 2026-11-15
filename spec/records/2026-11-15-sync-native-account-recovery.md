# Proof-bound native Sync account handoff recovery

15 November 2026. Owning Server change `wyqmxpnp`, SRV-SYNC-016.
Status: native Core/HTTP source boundaries qualified; protected custody and
application integration remain separate open gates.

## Before and behavior

The consuming native client previously returned an unknown outcome for a lost
redemption response, with no proof-bound way to inspect or revoke its issued
credential. Replaying redemption is forbidden. Account inspection was a
browser-owner operation and short-lived consent cleanup removed the request.

Native `/api/v3/sync/account/{id}/inspect` and `/cancel` operations accept only
the existing S256 verifier. The existing native protocol, 4-KiB body and
10-second admission apply; ambient browser cookies, bearers and foreign origins
remain refused. The safe reply contains only request/client identity, expiry,
request status and discovery-credential status. It discloses no proof, bearer,
authorization hash, account session or working-vault capability.

Issued associations persist separately from the ten-minute request and survive
cleanup and restart. Cancellation serializes with redemption under the control
store transaction: it prevents unconsumed issuance or revokes only that exact
handoff's issued credential. Legacy consumed requests without an association
report unavailable and cannot claim successful cancellation. The client checks
request/client identity and status combinations and refuses cancellation replies
that still show an active credential or an approved unconsumed handoff.

## Verification retained

Focused source commands were:

- `cargo +1.99.0 test --locked -p lapis-app-core --test sync_account_transport`:
  12 passed/0 failed, including two new recovery response validation cases.
- `cargo +1.99.0 test --locked -p lapis-server --test sync_account_recovery`:
  4 passed/0 failed: consumed lost response through cleanup/reopen and an actual
  fresh child process, cancellation before/after owner decision, concurrent
  cancellation/redemption with unrelated credential isolation, and unavailable
  historical associations.
- `cargo +1.99.0 test --locked -p lapis-server --test sync_account_http_recovery`:
  1 passed/0 failed, real bounded native route/proof and ambient-auth refusal.
- `cargo +1.99.0 test --locked -p lapis-server --test sync_native_account_transport`:
  1 passed/0 failed, current real HTTP handshake extended with exact inspection
  and cancellation of the issued discovery credential.
- Strict workspace all-target Clippy and specification/App-ownership checks pass.

The first widened workspace run failed the unchanged backup history reader
(53 passed/1 failed in `http_files`). The separate owning JJ Store fix and
[consumer intake receipt](2026-11-15-history-publication-consumer-intake.md)
retain that reproduction, the source repair and subsequent 54/54 target pass.
A later widened run stopped at an existing native queue completion deadline;
the exact target and its complete family then passed unchanged. The final
rebased workspace run below passes; every failed run remains retained.

## Qualification scope

These are macOS ARM64 native Core/HTTP source boundaries using task-owned
profiles, stores and subprocesses; no OS credential store was used. Durable
protected account orchestration, Keychain restoration, Tauri IPC and real
application admission remain open, as do endpoint/server enrollment, content
keys, encrypted transfer and all complete Sync journeys. Account authority
continues to be separate from endpoint and content authority. Nothing enables
Sync by default, replaces an installed application or deploys production.

Final non-overlapping locked workspace rerun passed: 862 passed/0 failed/
7 existing non-Sync ignores across174 target result summaries (exit0). Log:
`.local/sync-history-intake-workspace-final.log`. The original queue timeout
and exact/family reruns remain retained; no assertion, deadline or skip changed.
This result qualifies candidatec2f743c8 with the account source tree, before
upstream#86's new rebase. The final rebased source revision requires fresh
consistent consumer intake before publication or merge.

## Final rebased-source qualification

With JJ Store pin `87be3ad68530653c6c6fd1e5a2463a217cf3f044`, transport cases
pass 12/12. The control recovery, HTTP recovery and real native handshake
targets pass 4/4, 1/1 and 1/1. Logs are `.local/sync-account-final-pin-transport.log`
and `.local/sync-account-final-pin-server.log`.

`cargo +1.99.0 test --workspace --locked -j 2`: 862 passed/0 failed/seven
existing non-Sync ignores across 174 target summaries, exit 0. Strict all-target
workspace Clippy passes with warnings denied. Logs:
`.local/sync-account-final-pin-workspace.log` and
`.local/sync-account-final-pin-clippy.log`. No builds overlapped test execution.
Before that rebuild, with Cargo stopped, 170 superseded task-only executables
were retired (12,917,852,224 bytes); inventory remains in
`.local/sync-account-final-pin-retired-tests.json`.

A retrospective unchanged-regression check on the transport-only parent
`d0ec40ed`, adding only the HTTP recovery test file, fails 0/1: the route returns
404 where native admission expects 400. Log:
`.local/sync-account-recovery-parent-before.log`, test-only probe `zusuqmlv`.
This distinguishes the required new routes from a passing unrelated suite; it
is not represented as an earlier pre-implementation execution. The exact same
target passes 1/1 on the implemented tree; log
`.local/sync-account-recovery-parent-after.log`.
