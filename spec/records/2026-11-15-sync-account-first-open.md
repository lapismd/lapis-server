# First protected account open with locked custody

15 November 2026. SRV-SYNC-017. Source qualified; OS/Tauri integration remains open.

The exact `first_locked_custody_open_retries_after_unlock_without_replacing_identity`
regression failed0/1: the first locked custody lookup left `.owner.lock` without
`binding.json`, and the unlocked retry refused InvalidData rather than restoring
the original scope. Log `.local/sync-custody-first-open-before.log`.

Fresh ownership now reserves its public client/binding metadata before custody
lookup. The marker contains no secret and grants no account, endpoint or content
authority. A failed custody lookup still returns its sanitized error; no provider
is replaced and no credential is written. On unlock the same native identity
can be admitted. Existing locks with missing metadata/custody still require
recovery, and foreign/corrupt protected records remain refused.

The exact regression observes public metadata during the first custody callback,
verifies identical identity after unlock and runs an actual fresh native child.
Its explicit fixture custody never writes credentials and represents source
behavior, not an OS Keychain qualification.

Run exact and complete affected account targets, full workspace and strict
Clippy, protocol, specification and ownership checks before committing. Actual
OS, Tauri/App, enrollment, transfer/apply and full Sync remain open. Logs share
.local/sync-custody-first-open-\*. Historical receipts remain immutable.

The repaired exact target passes1/1 with zero ignores
(`.local/sync-custody-first-open-after.log`). It confirms the public marker is
already durable during the first failed callback, remains identical after
unlock, and restores that client in a fresh native child without any credential
write. The complete affected Core account families pass32/32 with no skips
(`family.log`, same prefix): protected state, session, owner, transport and the
new first-open target. Existing missing-state and foreign-record assertions
remain unchanged and pass. Current-server/cold-session and full workspace
qualification follow; fixture custody is explicitly synthetic.

The first canonical check refused five acceptance bullets under SRV-SYNC-017
against its two-to-four limit. The new public identity reservation detail is
now part of the existing profile-metadata acceptance concern; no invariant or
checker limit changed. Log `.local/sync-custody-first-open-spec.log`.

The second canonical check refused two sentences inside the combined public
metadata acceptance bullet; its final wording is one atomic sentence retaining
the same reservation and no-secret-write requirements. Log `spec-recheck.log`
with the same prefix; no validator changed.

## Final qualification

- Exact first-open regression:0/1 before,1/1 after, zero ignores; actual cold native child and identical public client identity.
- Complete Core account families:32passed/0failed/0ignored across five targets.
- Real current-server account family:6passed/0failed/0ignored, including cold native restoration and detached/uncertain account journeys (`server.log`).
- `cargo test --locked --workspace`:888passed/0failed/7existing non-Sync ignores across179result summaries, exit0 (`workspace.log`).
- `cargo clippy --locked --workspace --all-targets -- -D warnings`:pass (`clippy.log`).
- Canonical validation/mdBook/spec-first/App-ownership:pass (`spec-final.log`).

All Rust commands use task-owned .local/rust-core-target with pinned Rust1.99.0
on macOS ARM64; the custody port is an explicit test fixture, not Keychain.
The existing macOS large-unwind-table linker advisory remains visible. No source
assertion, timeout, ignore, identity-loss refusal, provider or protocol version
was weakened or replaced. Both failed specification attempts remain above.

Acceptance review: fresh locked lookup cannot strand the identity witness;
unlock and a new process recover the same public identity without credential
writes. Existing established missing-state recovery and corruption/foreign
record refusals pass unchanged. No account, endpoint or content authority comes
from reserving public metadata. OS custody, Tauri/App and all later Sync stages
remain separately unqualified.
