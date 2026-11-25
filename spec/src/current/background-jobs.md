# Plugin background jobs

This chapter owns the native job engine and host execution contract. Notes owns
the public authoring API and shared controls. Desktop owns Tauri admission and
application lifetime. Tasks owns its overdue digest handler.

## SRV-JOB-001 — Bounded headless execution

**Requirement.** An enabled plugin job MUST execute its verified headless module
in a fresh JavaScript runtime with bounded memory, stack, elapsed execution and
host calls. Native operations MUST pass through an admitted per-job broker.
Cancellation and deadline expiry MUST interrupt computation. The runtime MUST
expose no filesystem, process, network or renderer authority.

### Acceptance details

- Synchronous and asynchronous handlers complete through the same execution API.
- Denied operations, module imports, missing handlers and rejected promises fail.
- Infinite loops, excessive allocation and oversized calls terminate within bounds.

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

## Delivery status

The native implementation is under qualification. Desktop admission, native
metadata default cutover and both consumer journeys remain open in the
[background jobs backlog](../backlog/background-jobs.md).

## Hosted acceptance

The installed-plugin browser acceptance MUST use the real authenticated host and
shared production settings. It MUST prove disabled-by-default admission, manual
execution across browser detach, and private checkpoint restoration across a cold
server restart. A test deployment receipt does not replace signed release validation.
