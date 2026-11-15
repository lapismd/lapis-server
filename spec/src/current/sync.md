# Lapis Sync server

This chapter owns the shared native service and hosted destination boundaries
for Lapis Sync. Protocol algorithms remain in `lapis-plugins`; portable App and
session capability contracts remain in Notes; Tauri and OS adapters remain in
Desktop. Sync is optional and disabled until a vault explicitly enrolls this
server destination and participant.

The source-qualified foundation implements the Server-owned encrypted object
repository, opaque operation journal, dependency-ordered event/HEAD repository
and immutable arrival history under SRV-SYNC-004–007. They bind an admitted
physical profile, logical vault and destination, publish verified ciphertext
atomically, reuse stable retry bytes, refuse corruption, stream through a 256
KiB buffer, retain explicit operation outcomes, expose a device HEAD only after
its durable revision graph and provide stable bounded catch-up snapshots.
Protocol admission, enrollment, scheduling, working-file apply, HTTP and
application selection remain unavailable.

## SRV-SYNC-001 — One explicit destination per vault

<a id="srv-sync-001"></a>

**Requirement.** One local vault MAY select one Lapis Server destination. The
native service MUST bind every operation to the admitted profile, logical vault,
destination and participant identities. Missing or mismatched admission MUST
refuse without choosing another provider, persistence implementation, root or
renderer writer.

### Acceptance details

- Foreign profile, vault, destination and participant identities are refused.
- An absent destination leaves Sync unavailable and disabled.

## SRV-SYNC-002 — Separate authentication and content authority

<a id="srv-sync-002"></a>

**Requirement.** The server MUST keep account login, endpoint enrollment,
transport authorization, content keys, owner authority and recovery separate. An account
session alone MUST NOT decrypt content, enroll a participant, rotate membership
or authorize a vault mutation. Private keys, wrapping material, account bearers
and authorization verifiers MUST remain host-owned.

### Acceptance details

- Login grants neither decryption nor membership authority.
- Secret-bearing material never enters renderer snapshots or logs.

## SRV-SYNC-003 — Owner-authorized participant enrollment

<a id="srv-sync-003"></a>

**Requirement.** Pairing MUST require an authenticated owner decision over a
fresh, single-use request bound to the endpoint, vault, participant public key
and requested scope. Approval MUST issue only bounded transport authority and
the encrypted enrollment material selected by the owner. Expiry, rejection,
replay, vault mismatch and revoked authority MUST fail closed.

### Acceptance details

- Approval binds the exact request identity and current owner decision.
- Expired, consumed and revoked requests cannot be approved again.

## SRV-SYNC-004 — Durable encrypted destination ordering

<a id="srv-sync-004"></a>

**Requirement.** Encrypted objects and revision dependencies MUST become
durable and validated before a signed event or device HEAD makes them
discoverable. Destination state MUST distinguish staged, accepted and applied
work. A retry MUST reuse the persisted ciphertext and signed event identity;
an uncertain write MUST expose inspection instead of automatic replay.
Revision and HEAD arrival publication MUST follow durable event acceptance and
repair an interrupted local publication on reopen.

### Acceptance details

- Missing dependencies prevent HEAD discovery.
- Lost acknowledgements retain the original durable bytes and identity.
- A stale conditional HEAD does not replace the previously discoverable device tip.
- A stale conditional HEAD retains the new signed event as durable undiscoverable work.

## SRV-SYNC-005 — Native repository and filesystem isolation

<a id="srv-sync-005"></a>

**Requirement.** The server MUST keep Sync repositories, outboxes, apply journals,
wrapped credentials and private indexes under an admitted profile-private
root outside managed vault content. Filesystem effects MUST reject traversal,
symlink escape and root mismatch, use durable atomic replacement, and stream
large objects without exceeding the retained 64 MiB in-flight target.

### Acceptance details

- Process working-directory changes cannot retarget storage.
- Large-object qualification measures bounded streaming memory.

## SRV-SYNC-006 — Membership history and dependency validation

<a id="srv-sync-006"></a>

**Requirement.** The destination MUST validate signed event identity,
signature, attestation, vault scope, membership generation, authority cutoff,
object hashes, parents and archive references before acceptance. Revocation and
rotation MUST preserve earlier authorized history while refusing events first
authorized after the applicable cutoff.

### Acceptance details

- Unknown signers, generations and dependencies are refused without discovery.
- Rotation preserves history that was valid before its recorded cutoff.
- Caller-validated owner membership events retain an immutable generation and predecessor chain plus exact opaque bytes.

## SRV-SYNC-007 — Bounded catch-up without inferred deletion

<a id="srv-sync-007"></a>

**Requirement.** Event, object and membership catch-up MUST use bounded stable
paging with explicit cursor expiry and restart outcomes. An empty page,
incomplete page, expired cursor, missing key or unavailable participant MUST
NOT imply deletion, reset identity, overwrite drafts or discard recoverable
work.

### Acceptance details

- Cursor expiry returns an explicit restart outcome.
- An empty or partial page never creates a deletion.
- A page pins one arrival-sequence snapshot and excludes later arrivals until the caller starts a fresh catch-up.

## SRV-SYNC-008 — Continuous server participant

<a id="srv-sync-008"></a>

**Requirement.** An explicitly enrolled server MUST have its own device
identity and encrypted-content authority, capture its managed working vault and
continue participation without an open browser. Watch notifications MUST be
hints backed by startup and periodic reconciliation. Web sessions edit that
same server-owned working vault through existing admitted writers.

### Acceptance details

- External server edits are captured with every browser closed.
- Startup reconciliation repairs notifications missed while stopped.

## SRV-SYNC-009 — Checkpoint and scheduler policy

<a id="srv-sync-009"></a>

**Requirement.** Automatic checkpoint attempts MUST use a 20-second quiet
period, a five-minute maximum delay, single-flight ownership and bounded
retries. Local checkpoint durability MUST precede publication. Shutdown,
disable, vault retirement and profile switching MUST drain accepted work and
retain explicit pending or unknown outcomes without a browser-owned scheduler.

### Acceptance details

- Continuous edits cannot postpone an attempt beyond five minutes.
- Reconnect and duplicate hints cannot start concurrent checkpoint owners.

## SRV-SYNC-010 — Guarded apply and idempotent recovery

<a id="srv-sync-010"></a>

**Requirement.** Working-vault apply MUST hold exclusive admitted mutation
ownership, protect and flush active, inactive, embedded, retired and
multi-window writers, and persist an idempotent receipt before releasing the
guard. Interrupted apply MUST resume or report recovery from durable intent;
it MUST NOT infer success, replay blindly or overwrite a dirty draft.

### Acceptance details

- A durable apply receipt makes a repeated admitted command idempotent.
- Writer refusal preserves both the remote candidate and local draft.

## SRV-SYNC-011 — Scoped HTTPS destination API

<a id="srv-sync-011"></a>

**Requirement.** Production Sync transport MUST use the existing HTTPS policy,
same-origin owner consent where applicable and scoped bearer authorization.
HTTP requests MUST be size- and time-bounded, stream object bodies, reject
foreign vault/device scope before reading a body, and return stable sanitized
errors without paths, credentials, key material or plaintext content.

### Acceptance details

- Foreign scope is rejected before request-body consumption.
- Lost connections retain a stable inspectable native outcome.

## SRV-SYNC-012 — Independent native qualification

<a id="srv-sync-012"></a>

**Requirement.** Qualification MUST use independent stores, device identities,
keys and a real server to prove offline and cold-restart convergence, server
capture without a browser, stable retries, duplicate notifications, conflicts,
forward restore, authority changes, permission loss, corrupt dependencies,
cursor expiry, index rebuild and streamed large attachments. Source, HTTP,
hosted image, Desktop and installed-application evidence MUST remain distinct.

### Acceptance details

- A-to-Server-to-B and Server-Web-to-Desktop use independent stores and keys.
- Cold restarts and corrupted dependencies are exercised without warm-state reuse.

## Deferred destinations

Nostr, Blossom and S3 destinations, automatic legacy migration, multiple
destinations, live character collaboration and advanced garbage collection are
outside this contract. Their absence MUST NOT weaken the explicit Lapis Server
selection or preserve a hidden compatibility fallback.
