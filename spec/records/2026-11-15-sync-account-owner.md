# Retained native account action owner

15 November 2026. SRV-SYNC-019, enclosing Server Jujutsu change `ttunpkzs`.
Status: shared Core/current-server source qualified; Desktop and OS integration remain open.

Shared Core now owns account command admission and blocking execution. There is
one accepted action at a time and no queued command backlog. Busy refusal has no
effects. Accepted work owns the session until completion even if its async
response waiter is dropped. The host must retain AccountOwner through view
detach and refused teardown. The owner is not Clone; passive latest-value watch
receivers own no session, custody or worker lifetime.

Construction takes an existing session without IO and reports Uninspected. It
therefore retains even an already uncertain session. Commands inspect the fixed
scope through the existing coordinator on a native blocking worker. Failed
inspection publishes RecoveryRequired, never an old successful snapshot. A
confirmed close alone removes the session; a failed close retains its original
pending protected bytes and physical owner lock for explicit retry. Commands
carry no secrets, paths or provider selection. Device names are bounded before
acceptance; host adapters supply current time and native admission.

The initial exact availability probe failed compilation because account::owner
was absent. That probe is not functional qualification. Behavioral tests replace
it: detached waiter/single-flight/failed-close retention and passive observers
with no-runtime/invalid-name refusal initially pass 2/2. A third scenario covers
construction around an already uncertain session and recovery without resetting
its Prepared record. Final exact and broader results are recorded below when
complete. No network or OS custody qualification is claimed by these explicit
RAM-custody tests; real-server coordinator evidence remains in the prior receipt.

Node 26 search initially degraded because QMD native modules used another ABI.
Using task Node 24.15.0 restored exact SRV-SYNC-018 search; doctor passes ten
checks with the existing script-alias warning. No system installation changed.

Source logs use .local/sync-account-owner-\*. Wider execution must retain the
original boundary, all failures and actual counts. Task-owned executable
retirement is recorded in sync-account-owner-retired-tests.json; no source,
fixtures, credentials or unrelated target roots are removed.

OS Keychain, Desktop admission/Tauri/App integration, end-to-end Sync, platform,
installed and release qualification remain open. No activation, publication,
production deployment or installed application replacement is authorized here.

The first canonical check refused seven acceptance bullets under SRV-SYNC-018
and three multi-sentence bullets. Ownership is now its own SRV-SYNC-019 contract
with four atomic acceptance cases; the existing coordinator contract is retained.

The next spec check found the new verification row outside its table; joining
it to the existing table fixes mapping without changing validator policy.

Final exact Core owner scenarios pass 3/3 with no skips; the first 2/2 iteration
and the missing-module compilation probe are retained separately. The current
real-server family adds detached handoff completion, lost redemption inspection,
refusal of a second redemption, exact revocation and original-client restoration.
Its existing cold-process journey now consumes AccountOwner for restored
snapshot/discovery/close, rather than qualifying only AccountSession.

Published spec-validator remains latest 0.2.0, confirmed through pnpm registry
metadata; no dependency update is required. Protocol checks pass. The corrected
full specification lane passes validation, mdBook and spec-first, followed by
zero App ownership findings. Strict Core all-target Clippy passed before the
final additional scenario; final workspace lint is still required.

Final focused qualification: `cargo test --locked -p lapis-app-core --test
sync_account_owner` passes 4/4 (expanded-final log), and `cargo test --locked -p
lapis-server --test sync_account_session` passes 6/6 (server-final log), including
the actual fresh native child and the detached owner/lost-redemption journey.
The fourth Core case proves locked custody replaces Ready with RecoveryRequired,
sanitizes the private error and restores the identical client without writes.
`cargo clippy --locked --workspace --all-targets -- -D warnings` passes after
those final scenarios. Commands use the task's .local/rust-core-target, never
an installed application. The macOS test linker retains its existing large
unwind-table advisory; strict Rust lint itself reports no warnings.

The complete locked workspace exits zero: 885 passed, zero failures, seven
existing non-Sync ignores across 178 result summaries (workspace.log). This
build collected three Core owner cases and six Server coordinator cases; the
final expanded Core target separately passes all four cases, and the final
Server target separately passes six including the updated cold child. Production
owner code is identical across these runs; later edits expanded tests only.
No zero-test target or ignored case is counted as functional qualification.
Workspace Rust formatting passes. Final specification and changed-document
formatting are rerun after recording these results.

Acceptance review: all four SRV-SYNC-019 source cases are covered, while host
retention through actual Desktop reload/close and OS custody remain explicit
consumer gates. Prior protected-state/session receipts remain immutable; no
account or content authority is broadened. This closes only the bounded Core
owner slice, not the full Sync roadmap or optional activation gate.
