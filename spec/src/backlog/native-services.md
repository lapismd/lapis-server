# Native application services

These are retained feature gaps from the shared Rust and hosted-parity plans.
They are not permission to enable staged services or revive the retired Node
server/companion architecture. lapis-server owns portable application policy and
its core/protocol crates; generic engines remain in lapis-ai, jj-store and
lapis-plugins. Shared App/API contracts remain in Notes.

## Lapis Sync native service and destination

Disposition: contract established in [Lapis Sync server](../current/sync.md);
implementation and every native/application acceptance boundary remain open.
The discarded Stage 3 through Stage 7 draft stack is provenance only: its
process-relative object writes, incomplete authorization, missing HTTP
destination, oversized modules and absent canonical mapping do not qualify a
source foundation.

Acceptance: implement the shared durable repository, protected working-vault
participant, scoped enrollment and destination API in bounded slices. Prove the
exact security, retry, paging, scheduler, writer, recovery and independent-store
journeys in SRV-SYNC-001 through SRV-SYNC-012 before hosted activation. Desktop
custody/admission and Notes/plugin consumption remain separately owned.

The first clean Core slices now provide the disabled profile-private encrypted
object repository, [opaque operation journal](../../records/2026-11-15-sync-operation-journal.md)
and [dependency-ordered event/HEAD storage](../../records/2026-11-15-sync-event-ordering.md).
The [arrival repository](../../records/2026-11-15-sync-arrival-catch-up.md)
adds immutable membership history and bounded stable event catch-up with explicit
cursor-expiry recovery. These slices do not advance protocol admission,
enrollment, scheduling, guarded working-file apply, HTTP, hosted participant or
independent-device acceptance.

The [account consent store](../../records/2026-11-15-sync-account-consent.md)
now retains expiring S256 requests and revocable discovery-only credentials.
Bounded native initiation/redemption and owner-cookie decision routes now exist;
managed-vault discovery exposes only names and identities, while sign-out
revokes discovery-only credentials. The
static hosted consent screen now supports owner sign-in, request inspection,
keyboard decisions and explicit unknown-outcome recovery. Desktop custody and
intake plus every endpoint/content enrollment boundary remain open. The shared
native account HTTP client now has fixed-origin HTTPS admission, bounded
handoff/discovery, secret-bearing host-only values and real current-server
qualification under SRV-SYNC-015. Protected credential restoration and durable unknown-outcome orchestration now
have source evidence under SRV-SYNC-017/018. Shared Core account ownership adds
single-flight bounded blocking execution and passive observations; Desktop
admission, Tauri consumption and actual OS qualification remain open. Proof-bound
handoff inspection and exact cancellation under SRV-SYNC-016 now preserve a
consumed credential association through consent-request cleanup and restart;
legacy missing associations stay explicitly unavailable. These account-only
routes do not enroll an endpoint or authorize working-file mutations.

## Logical participants and rich queue workflows

Disposition: remaining implementation and host qualification. Native publication,
typed transport, declared profiles and shared observation have historical source
receipts. Profile editing, busy group queues and complete real-provider workflows
still need feature-specific proof. Tauri profile integration belongs to Desktop.

Acceptance: preserve one authored message, captured dispatch/binding/delivery
identities, independent participant outcomes and one conversation publication
owner. Simultaneous workers cannot replace one another's output. Prove bounded
FIFO, competing clients, capacity, lost replies, cancellation/Stop and detached
completion. Cold recovery remains paused until explicit resume; uncertain work
is observed without replay. Keep accepted-call drain and caller/vault identity.

## Context, commands, skills and handoff

Disposition: context/bootstrap/skill foundations have source receipts; rich
dispatch, command and handoff assembly and complete actual-host integration need
separate completion. Retain immutable instructions and negotiated modes, bounded
dispatch-time attachment reads/hashes, versioned skills and authored/context
separation. Explicitly identify client-dependent capture.

Acceptance: prove both transports with retained public screens, stale bindings,
changed files, cancellation, detach and cold recovery. Persisted transcript and
reconnect logs exclude raw startup, profile, handoff, attachment and recall
context previews. Present-day settings cannot reconstruct historical context.

## Native approvals, tools, Changes and undo

Disposition: remaining native feature parity. Reuse existing runtime, file and
repository engines; do not introduce arbitrary JavaScript execution or an
unavailable-client fallback. Preserve exact offered approval IDs, freshness,
request correlation, current policy and owner-only answers across competing tabs.

Acceptance: bound tool concurrency and enforce source authorization, writer and
dirty-editor guards. Confirm a saved-file baseline before writes and terminal
capture after admitted writers drain. Changes are observations, not exclusive AI
authorship. Guard selected-path/turn undo with exact pins, ending bytes/modes,
safety checkpoints and per-path recovery. Baseline failure offers explicit retry,
cancel or continuation without protection. Disabled history never initializes
storage; synced links do not grant undo authority. Qualify cancellation, unknown
effects, detach and cold restart through both hosts. Imported receipt rebuilding,
hunk/merge-aware undo and whole-conversation undo remain deferred.

## Native memory coordination

Disposition: deferred. Native storage and existing Markdown memory remain;
service-owned capture/recall, claims, maintenance, forget/relocation and unattended
cold recovery are unfinished. Preserve existing content and handwritten edits.

Acceptance: capture only after durable terminal turns; preserve scope, source
provenance, saved timezone, conditional writes and dirty editors. Forget/exclusion
receipts survive index rebuilding without recreating forgotten material. Folder
relocation is recoverable without silent overwrites or merges. Restricted
background processing uses a qualified bundled profile without tools or filesystem
discovery. Qualify competing clients, disconnect and cold recovery independently
of a visible memory UI.

## Durable operations and explicit imports

Disposition: deferred. Move accepted import/capture, Forge/transfer/Compute,
installation and terminal operations to service-owned lifetime/progress/cancel/
recovery by adapting existing engines. Do not rebuild them or add a window-owned
scheduler.

Acceptance: retain Codex JSONL, Cursor transcripts and OpenCode SQLite source
formats, bounded progressive readers, opaque handles, cancellation/drain and
deduplication. Provisional previews cannot be saved; publication requires unchanged
file fencing or a pinned SQLite transaction. Sources are explicit uploads or
operator-admitted data, never an unrestricted home-directory scan. Private indexes
exclude bodies/credentials and remain rebuildable. Automatic capture stays a
separate opted-in Codex feature. Installation transactions, transfers and terminal
ownership must survive observer detach and expose truthful interrupted/unknown
outcomes without automatic replay.

## Optional legacy History ownership

Disposition: deferred separately from supported native repository history. Current
Desktop and hosted profiles do not bundle the optional legacy AppDatabase History
plugin. Its installed capture remains client-dependent. A native scheduler needs
its own optional contract, lifecycle and host acceptance; current-profile cutover
does not require enabling it.

## Optional Docker agent execution

Disposition: deferred. Host-process execution remains the initial backend.
An eventual generic lapis-ai execution port must cover trusted image selection,
admitted mounts, path translation, network/secret delivery, resource limits,
protocol streams, cancellation, status and confirmed cleanup. Provider/model/mode,
approval policy and process placement remain independent. An unavailable backend
refuses without fallback. Killing an attached client cannot prove container
cleanup; advertised capabilities must match the backend actually implemented.

## Provenance and dependencies

Former Rust stages for AI, memory and operations and hosted-parity slices are
consolidated here by behavior. The
[Notes reconciliation](https://github.com/lapismd/lapis-notes/blob/main/spec/records/2026-10-30-spec-consolidation.md)
retains their original identities and receipt pointers. Shared runtime/core
interfaces must be updated in Notes before or with implementations. Source and
consumer qualification remain distinct; publication requires its own authority.

## Backup initialization failure diagnosis

Disposition: unresolved Desktop Original/Managed reader failure. The
[initialization receipt](../../records/2026-11-02-backup-initialization-diagnostics.md)
added opt-in debug initialization stages. A fresh hosted Linux execution then
completed every initialization stage but failed the explicit stop at final
capture; the [shutdown receipt](../../records/2026-11-03-backup-stop-diagnostics.md)
adds safe stop-stage categories for the next original execution. Local Linux
exact and scoped consumer cases pass and are non-reproductions, not proof of a
fix. Preserve Desktop's original ARM64 quarantine, independent histories,
source bytes and start/stop assertions. Execute the original Desktop case alone
and in its integration-test group on ARM64, preserving first failures and
bounded diagnostics. Diagnose the reported owning stage, fix the demonstrated
cause and qualify the unchanged scenario before removing the quarantine. Never
include unrelated ignored subprocess helpers.

## Repository remotes and credentials

Disposition: approved shared UI preview; native transport and encrypted custody
foundations are in progress under SRV-SRV-015/016. Integrate both targets through
Core/Protocol and authenticated host adapters, keeping desktop OS custody and
server encrypted custody separate. Saving never transfers; anonymous HTTPS and
host-managed SSH do not require a forge account. Preserve original JJ bookmark
semantics and existing backup recovery records.

Shared manual original transfers now have source and real SSH acceptance in
[the Core receipt](../../records/2026-11-15-original-transfer-orchestration.md).
Shared backup operations additionally have real SSH proof for immutable reviews
across capture, standby fetch and restart/stop recovery in
[the backup receipt](../../records/2026-11-15-backup-transfer-orchestration.md).
Shared credential-provider mutation/sign-in and the encrypted Server adapter
now have source qualification. Server installation startup and guarded native
HTTP routes have encrypted restart and real HTTP/SSH qualification. The [cold compiled HTTPS receipt](../../records/2026-11-15-cold-server-https-remotes.md)
qualifies actual binary token/anonymous transport, one branch, detach, restart and
locked-key custody. Browser/forge UI and Desktop cold acceptance remain separate. Native automatic
backup scheduling, passive status and explicit recovery now have [Core/HTTP and
actual SSH qualification](../../records/2026-11-15-native-backup-synchronization.md).
Complete the Desktop OS adapter and routes, shared observer intake and both browser/native application
compositions, then qualify the approved production components. The SDK intake must retain its
capture-independent transfer qualification and merge proof; do not add consumer
Busy retries.

Acceptance: qualify credential replacement/forgetting, locked/mismatched keys,
restart persistence, endpoint/owner isolation, redaction and key provisioning;
then real HTTPS/SSH fetch and one reviewed branch push, rejection and uncertain
outcome recovery in each cold host. Native source tests do not enable routes or
qualify Desktop/hosted delivery. GitHub publication follows completed owner and
consumer qualification with the user's standing release approval.
