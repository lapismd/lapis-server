# Repository remote operations

These details implement SRV-SRV-004 and the credential requirements
[SRV-SRV-015/016](server.md#srv-srv-015). Host and source qualification follow
the [repository rules](governance/remotes.md).

Core owns target-aware private remote selection independently of secret custody
and repository configuration. Saving a new or changed backup connection MUST
leave automatic synchronization off. Original selection never changes that
backup preference. Exact configuration revisions and pending-publication guards
fence stale writes; records contain validated endpoints and opaque credential
references, never token bytes or host filesystem paths.

Opted-in backup synchronization MUST be owned by Core for the admitted vault,
survive client detach, and drain accepted work at close without retaining an idle
work lease. It MUST use native capture, merge proposals and guarded apply before
single-branch publication. Conflicts and uncertain apply receipts MUST pause for
explicit recovery; restart MUST NOT replay an uncertain file application.
Passive synchronization status MUST remain observable during network work.
Original-repository transfers remain manual. See [qualification](governance/remotes.md).

Forge credential ciphertext uses a separate authenticated domain and the full
256-bit account identity, alongside installation and owner. Account metadata
and token inventory remain independent. Replacing/forgetting forge secrets
preserves the stable account identity; no stored secret crosses into browser
inventory. Missing keys and authentication failures never create plaintext
fallback records. Host forge-manager and route adoption remain separate gates.

Shared remote commands MUST acquire native accepted-work ownership before
waiting for their serialized turn. Discarding a response or detaching its client
cannot abandon an accepted private configuration write; delivery is fenced after
completion. A later admitted client MUST use the same vault state without being
bound to the retired client. No per-vault remote owner may retain a permanent
work lease that prevents vault closure. Settings projection grants no transport
or original mutation authority.

Working-copy status MUST read an admitted existing Git-backed JJ original
without snapshotting files, changing checkout metadata or moving history pins.
Its frame identifies that source and includes complete bounded file/line totals;
unsupported or unstable observations remain unavailable rather than invented
zeros. With no original, an already initialized managed backup MAY report its
latest recorded checkpoint and native parent-diff totals. This status MUST omit
later unrecorded disk bytes, use the existing native diff engine, and retain
unknown line counts for binary/unsupported content. It MUST NOT initialize a
repository, start capture, move a pin or silently fall back after an invalid
original-source witness. Native source witnesses remain independent of mutation capabilities and client-owned
history handles.

Passive original remote discovery MUST use the immutable native constructor,
without obtaining a mutation capability from a history/source reader. It lists
existing remote names and sanitized optional URLs and eligible local bookmarks;
credential-bearing or unsupported URLs never cross the host boundary. Discovery
neither selects `origin` nor writes Git configuration, initializes transport,
enables synchronization or advertises fetch/push authority. Existing clients
may omit the additive discovered-remotes field.

Explicit original connection saves MUST bind private selection to the witnessed
physical source and exact discovered Git configuration revision. External
configuration changes advance the private observation epoch and refuse stale
settings writes. The native Git configuration transaction runs under the private
selection process lock, after credential scope and pending-publication checks.
If Git config succeeds but private persistence fails, the previous selection
cannot authorize a mismatched endpoint or source. Unrelated remotes survive;
saving does not initialize transport, transfer data or change working-copy state.
Anonymous HTTPS and host SSH remain valid with locked token storage; missing,
wrong-host or unavailable token/account references refuse before Git writes.
Only explicit accepted saves or separately admitted manual transfer commands
construct the separate native writer capability; passive source witnesses and
History handles never acquire it.

Native host composition MUST explicitly admit transfers independently of
credential inventory. Test, fetch, push review, execution, observation and stop
commands MUST hold the private cross-process selection guard for their accepted
operation. Stale revisions, source/configuration changes and invalid review or
request identities refuse before credential resolution or network access.
Test connection observes remote reachability/authentication without fetching or
publishing. Anonymous reads and host SSH need no unlocked token store.

Original fetch MUST use native JJ bookmark/tracking semantics without capturing
files, writing checkout state or retargeting a client-owned History pin. Status
after a metadata-only fetch MUST remain readable through the qualified native
immutable observer; application code MUST NOT repair the checkout pointer or
substitute invented counts. Observations retain real bounded incoming/outgoing
counts and their fetch time independently for the selected connection.

Original push reviews MUST persist outside the vault, bound to the physical
source, exact saved connection and native immutable review. The public response
contains only an opaque review identity, selected commit and safe summary/counts.
Execution consumes that identity durably before invoking native publication,
which revalidates its operation, endpoint, bookmark and ancestry and reserves a
stable receipt before network publication. Consumed reviews cannot replay.
Pending publication MUST survive detach/restart, prevent new publication or
rebinding, and expose its original identity for observe-only recovery or an
explicit local stop. Stopping requires no credentials and cannot claim to undo a
possibly accepted remote update. Native custody resolves scoped secrets only
inside the accepted backend operation; stored secrets never become DTO fields.

Managed backup test/fetch/review/publication/recovery MUST use the same explicit
target and private selection admission, independently of original repositories.
Fetched metadata MUST enter through the running native capture worker when one
exists. With no running worker, explicit metadata import MUST NOT start capture
or snapshot files. Network transfer and immutable review/validation MUST remain
independent of the capture lease; application code MUST NOT mask native races
with Busy retries.

Backup review MUST pin an existing recorded checkpoint and native intent/counts.
A later local capture cannot replace that reviewed commit or add newly saved
files to its publication. Missing local history remains unknown and cannot be
reported as zero outgoing changes. Source/connection-bound private journals and
their bounded storage remain independent for the two targets. Native pending
receipts survive restart, prevent rebinding/new publication, and permit
observe-only recovery or an explicit credential-free local stop. New backup
connections and later local captures MUST remain unpublished until an explicit
push or separately persisted synchronization opt-in.

Credential mutations MUST execute through backend-only custody with the same
accepted-work lifetime and an installation-scoped process fence. An observation
epoch MUST be durably reserved before provider effects; every vault MUST observe
that epoch before admitting another command, fencing stale credential settings
across targets and vaults even after a lost mutation response. Remote selection
records contain only the epoch and opaque references, never submitted secrets.
Replacement MUST preserve endpoint/account scope. An unresolved publication MUST
retain its referenced credentials; terminal native outcomes retire its review
hold without permitting replay. Sign-in completion that could replace a retained
account MUST wait for observation or explicit local stop.
Custody futures MAY be single-threaded inside the accepted native executor;
they MUST NOT cross the worker/task boundary or retain a renderer lifetime.

Native credential composition shares account inventory, endpoint/account scope,
identity-preserving token replacement and SDK device authorization in Core.
Hosts inject their admitted-owner raw-token store and forge manager with private
metadata and separate secret custody. No renderer request chooses a store root.
A locked provider retains host SSH capability but disables token/account access;
private device authorization sessions never cross the Protocol boundary. Saved
forge identity comes from the verified provider, not submitted display fields.
