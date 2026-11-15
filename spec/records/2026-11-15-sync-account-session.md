# Protected native account coordinator

15 November 2026. Owning Server change `nrwuxzqx`, SRV-SYNC-018.
Status: native Core/current-server source qualified; OS custody and application
integration remain open.

## Behavior

Shared Core now owns a bounded blocking account coordinator over the explicitly
selected protected store and fixed-origin account endpoint. It has no renderer,
Tauri, browser session, application scheduler or persistence fallback. Its public
snapshot is an allowlist of nonsecret identity, phase, expiry, consent URL and
safe-abandonment fields; host records and the service have no Debug or public
serialization implementation. Proofs and discovery bearers use zeroizing fields.

Preparation, initiation, redemption and sign-out intent are protected before
network mutation. A protected-write error fences further work and close until
an identical-byte, inspect-first custody retry succeeds. Preparation confirmed
without network work can resume; initiation lacking a returned request identity
remains quarantined for 670 seconds (request lifetime, transport bound and clock
allowance) before explicit abandonment. Client identity remains unchanged.

Consent is inspected before one redemption attempt. A lost response retains
Redeeming with its proof; redemption cannot be replayed. Explicit disconnect
revokes that exact credential through proof-bound cancellation. Ordinary
connected sign-out uses the discovery bearer after persisting intent. Lost
sign-out/cancellation replies are inspected before another mutation, and a
confirmed terminal state clears local authority without another request. Unknown
historical associations remain retained. Expired/corrupt state never silently
becomes a new disconnected identity.

Network-uncertain states are durable and can close/reopen for later inspection;
unconfirmed protected writes refuse close. This source coordinator provides no
view-owned scheduler or alternative mutation owner.

## Before and focused verification

The initial module-availability probe failed compilation because the session API
was absent; it was replaced by behavioral tests and is not qualification.
Core's initial three behavioral tests passed. Expanded record validation then
failed 3 passed/1 failed: Serde's unit Disconnected variant accepted an unknown
field despite deny_unknown_fields. The exact record regression failed 0/1 before
repair and passed 1/1 after using an empty struct variant with the same wire
shape. The regression also covers version, client, origin, unknown phase and
invalid proof. Logs end in `record-{before,after}.log`.

The first real-server cold-process target failed 0/1 because its fixture selected
127.0.0.1 for insecure browser cookies; the existing localhost-only policy refused
it. Correcting only the test origin yielded 1/1. No policy was relaxed. Initial
real-server family passed 4/4, and the additional lost-cancellation target passed
1/1. Final focused coverage passes 5/5 real-server coordinator cases, 17/17
across all six Server account targets and 54/54 across seven Core Sync targets.
The public safe-abandonment deadline has its own exact passing scenario.

The real current-server journeys cover protected pending-consent reopen,
owner-approved discovery, a fresh native child process restoring the same
connected client without browser credentials, saved-client sign-out/reopen,
lost redemption response with one redemption only, lost sign-out response with
one sign-out only, lost cancellation response without a second cancellation,
orphaned initiation refusal/explicit bounded abandonment and a lost custody
acknowledgement after credential persistence. The public snapshot key allowlist
is asserted. Fault injection removes the body only after the real handler
commits; it is response-loss source evidence, not TCP packet-loss qualification.

Custody fixtures are explicit fakes: RAM in Core tests and a test-only JSON file
outside the disposable profile/vault in process tests. The cold process receives
only a public scope descriptor path through its environment; proof/bearer bytes are not
environment variables, arguments or public observations. Those fake bytes are
cleaned with the task-owned temporary root and do not qualify OS protection.

## Remaining gates

OS credential custody, locked/revoked Keychain, Tauri/window/profile admission,
application lifecycle, installed consumers and complete independent-device Sync
journeys remain open. Account authority still grants neither endpoint enrollment
nor encrypted content access. No production profile, installed application,
package publication, image deployment or visual baseline is changed.

Strict workspace Clippy initially refused one collapsible-if in record validation.
The equivalent let-chain form repairs that diagnostic without an allowance;
the exact strict workspace all-target command then passes with warnings denied.
Before the broad rebuild, with Cargo stopped, 174 obsolete task-only test
executables (12,449,310,400 bytes) were retired using the Cargo metadata inventory;
`.local/sync-account-session-retired-tests.json` retains every path and size.
The complete locked workspace passes 881 tests/0 failures/7 existing non-Sync
ignores across 177 result summaries, exit 0. Log:
`.local/sync-account-session-workspace.log`. The macOS test linker emitted its
existing large-unwind-table warning; it is distinct from Rust lint diagnostics.

The final Rust formatting check found only the new module declaration out of
alphabetical order. Formatting repaired that source-only ordering; the final
Core session target, real Server session target and strict workspace all-target
Clippy were rerun afterwards. There is no changed behavior or assertion.
Protocol Rust/TypeScript binding checks, canonical/mdBook/spec-first/App audit,
Prettier and changed Rust formatting pass. Doctor passes ten checks with one
existing script-alias warning. Final logs share the `sync-account-session-`
prefix under `.local/`; before/failure logs remain retained.
