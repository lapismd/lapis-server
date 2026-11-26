# Plugin background jobs

This chapter owns the native job engine and host execution contract. Notes owns
the public authoring API and shared controls. Desktop owns Tauri admission and
application lifetime. Tasks owns its overdue digest handler.

## SRV-JOB-001 — Bounded headless execution

**Requirement.** An enabled plugin job MUST execute its verified headless module
in a fresh JavaScript runtime with bounded memory, stack, elapsed execution and
host calls. Native operations MUST pass through an admitted per-job broker.
Cancellation and deadline expiry MUST interrupt computation. The runtime MUST
expose no ambient filesystem, process, network or renderer authority. Explicit
network grants use SRV-JOB-009. A settled native
operation MUST NOT permit successful completion after deadline expiry or
cancellation, including handlers that catch callback errors. Admitted native
operations MUST settle before the execution lease ends.

### Acceptance details

- Synchronous and asynchronous handlers complete through the same execution API.
- Denied operations, module imports, missing handlers and rejected promises fail.
- Infinite loops, excessive allocation and oversized calls terminate within bounds.
- Settled native callbacks cannot mask terminal cancellation or deadline expiry.

## SRV-JOB-002 — Calendar scheduling

**Requirement.** Jobs MUST support bounded fixed intervals and daily schedules
with an explicit IANA time zone. Missed schedules MUST collapse to one latest
occurrence. A repeated civil time MUST have one occurrence. A spring clock gap
MUST select the first valid minute after the requested time.

### Acceptance details

- Daily schedules retain their named timezone across UTC and local-date boundaries.
- Missed intervals collapse and daylight-saving transitions retain one date identity.

## SRV-JOB-003 — Durable native ownership

**Requirement.** Native owners MUST persist configuration, grants, checkpoints and bounded run history outside vault content. Accepted work MUST survive client detach. Explicit vault close and shutdown MUST fence admission and drain work. A cold running receipt MUST become interrupted and disabled until explicit acknowledgment.

### Acceptance details

- Checkpoint data stays outside the content root and survives reopening.
- Closing a vault waits for counted work; cold running records require inspection.

## SRV-JOB-004 — Installed code and permissions

**Requirement.** Each invocation and broker call MUST revalidate enabled installed plugin declarations and verified bytes. Execution MUST require explicit matching fingerprint approval, workspace trust and inactive Safe Mode. Plugin replacement, revocation, pause and policy fencing MUST stop further admission and cancel running computation.

### Acceptance details

- Changed verified bytes and native trust/Safe Mode fencing refuse admission.
- Requested grants cannot exceed the installed declaration.

## SRV-JOB-005 — Guarded vault operations

**Requirement.** Job operations MUST use admitted native metadata and editor-aware conditional frontmatter writes. Approved fields MUST bound mutations, preserving note bodies and unrelated properties. Dirty editors, path escapes and stale versions MUST refuse writes. Uncertain writes MUST require interrupted-outcome inspection before re-execution.

### Acceptance details

- Native frontmatter changes preserve body bytes and unrelated YAML properties.
- Dirty editors and stale versions refuse mutations without an alternative writer.

## SRV-JOB-006 — Notification delivery

**Requirement.** Notifications MUST persist through the existing Lapis history with bounded plugin-job deduplication keys reserved before delivery. Optional native alerts MUST require a host delivery sink and explicit configuration. Alert failure MUST preserve persistent history and report failure without automatically replaying delivery.

### Acceptance details

- Repeated notification identities do not deliver twice within retained history.
- A failed system alert preserves the persistent Lapis notification.

## SRV-JOB-007 — Runner handoff

**Requirement.** A runner MUST durably disable a job before issuing a destination-bound handoff. Receipts MUST carry checkpoints, schedule high-water and notification keys, and bind both installation and vault identities. Destination import MUST validate matching installed code and reject replay. Imported work MUST remain paused until explicit approval.

### Acceptance details

- Different destination vault UUIDs are accepted only when bound by the source receipt.
- A repeated import is rejected and imported jobs remain paused.

## SRV-JOB-008 — Schedule admission

**Requirement.** Each job MUST run single-flight and persist its running receipt before execution. Automatic scheduling MUST collapse missed occurrences and preserve a monotonic scheduled-time high-water through wall-clock rollback and restart. Persistence failure MUST fence the scheduler instead of starting unrecorded work.

### Acceptance details

- Concurrent manual and scheduled triggers cannot start another invocation.
- Clock rollback cannot repeat a recorded scheduled-time high-water.

## SRV-JOB-009 — Scoped network requests

**Requirement.** Job network requests MUST require exact-origin, method and
private-network approval within the verified declaration. Native requests MUST
validate and pin every resolved address, enforce deadlines and byte limits, and
respond to cancellation and policy revocation without retaining the catalog
lock. Ambient proxies, cookies and automatic redirects MUST be unavailable.
Link-local, metadata and special-use addresses MUST remain denied.

### Acceptance details

- Public endpoints require HTTPS; explicit private grants may admit local HTTP.
- DNS changes, mixed safe/unsafe answers and cross-origin redirects cannot widen grants.
- Pause, removal, trust fencing and deadlines cancel DNS, connection and body reads; stalled DNS must permit prompt executor shutdown.

- DNS uses asynchronous native sockets and fully qualified names, without search-suffix widening or a second terminal dot on already-qualified names.

## SRV-JOB-010 — Opaque background credentials

**Requirement.** Job credentials MUST remain in installation-native custody,
scoped to the admitted vault, plugin and job. Explicit bindings MUST restrict
native injection to an approved origin and header. The control API MUST accept
write-only replacement/removal and return configured status, never stored
values. Guests MUST NOT read credentials. Client detach MUST preserve native
custody; handoff MUST NOT copy secrets.

### Acceptance details

- Host administrator admission and job-owner approval are independent requirements.
- Locked or missing custody fails closed without printing keys, URLs or bodies.
- Cold recovery, rotation and another vault/plugin/job prove scope isolation.

## SRV-JOB-011 — External-effect recovery

**Requirement.** An uncertain mutating network request MUST leave the run
interrupted and disabled, including when a guest catches its error. Failure or
cancellation after an external mutation MUST require owner inspection before
re-execution. Native requests MUST settle before releasing counted work. Safe
read failures MUST NOT be automatically promoted to successful runs.

### Acceptance details

- A response lost after dispatch never causes automatic mutation replay.
- A successful external mutation followed by a handler exception remains interrupted.
- Read-only fetches can update checkpoints through the existing durable job state.

## Delivery status

The [terminal completion receipt](../../records/2026-11-25-plugin-job-completion.md)
retains the exact cancellation/deadline regressions and full Rust retry.

The native implementation and real hosted journey are
[registry-qualified](../../records/2026-11-24-plugin-background-hosted.md).
[Public dependency adoption](../../records/2026-11-25-background-registry-adoption.md)
is merged. Desktop admission/default cutover and released Tasks acceptance are
closed by the [v1 verification receipt](../../records/2026-11-26-background-job-verification-closure.md).
Actual macOS system-alert acceptance and the separately requested network and
credential extension retain their own gates in the
[background jobs backlog](../backlog/background-jobs.md).

## Hosted acceptance

The installed-plugin browser acceptance MUST use the real authenticated host and
shared production settings. It MUST prove disabled-by-default admission, manual
execution across browser detach, and private checkpoint restoration across a cold
server restart. A test deployment receipt does not replace signed release validation.
