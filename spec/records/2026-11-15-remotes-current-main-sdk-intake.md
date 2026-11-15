# Combine current-main history fixes with repository remotes

Server main advances from `a32ab23d` to `ce6ee5e6` while remotes qualification
and App Profile authentication are in progress. The new main intake selects
jj-store `87be3ad6`; the remotes branch selects `6649b7e9`. Those Git revisions
diverge from `6fdce65c`, so choosing either side silently would discard a required
source repair. GitHub correctly reports the feature PR as conflicting against
the new main and cannot run pull-request CI until integration is resolved.

The combined intake selects current published Git source
`086def182eae2ab6e5dc39aba030390d7ea8612c`. It descends from the remote credential
replacement source `6649b7e9` and includes the subsequently rebased immutable
history-frontier repair and its follow-up qualification. Core and Server direct
JJ dependencies, Cargo lock, manifest guard and canonical SRV-SRV-002 use that
single reachable source. Existing remote settings, encrypted custody and native
synchronization implementations are retained. The concurrent main Changeset and
immutable historical consumer receipt remain intact.

The first lock update uses an invalid `cargo update --git` flag and performs no
update. The corrected `cargo +1.99.0 update -p jj-vault --precise
086def182eae2ab6e5dc39aba030390d7ea8612c` resolves the pinned source without a
path override or blanket registry refresh. The native fixture stays on Rust
1.99 and the isolated pinned Linux validation image recorded in the earlier
native synchronization receipt.

The original hosted backup exact case passes 1/1 on this combined source before
the full suite. Native source snapshot `b18f01bc6b45af6a9f88fdb7c816764c98af97b0` passes all
905 workspace cases with zero failures and 11 explicit opt-in ignores across
182 result summaries. Strict all-target workspace Clippy passes. All four actual
SSH cases pass: original selected-bookmark publication, backup review/capture
independence, vault-owned synchronization and authenticated Server transport.
Disposable agents, endpoints and keys are retired by the harness. Specification,
App ownership and the manifest guard pass; normal workspace build and final
repository formatting checks pass. The source changes do not qualify
Desktop Keychain custody, cold browser/native application acceptance or HTTPS
provider sign-in. Their release gates remain open.
