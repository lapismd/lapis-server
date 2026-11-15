# Plugin operations

Compiled providers share the native registry and hosted transports. Outstanding
Desktop, installable-provider and delivery work belongs to the content automation backlog.

## SRV-OPS-001 — Shared operation registry

**Requirement.** Native plugin operations MUST register namespaced, versioned descriptors and typed handlers in one transport-independent registry. HTTP, MCP and CLI discovery MUST derive from those descriptors. Duplicate identities, incompatible CLI mappings and reserved command roots MUST fail registration. A second provider MUST require no transport-specific dispatch changes.

### Acceptance details

- Two providers share the same registry and dispatch implementation.
- Invalid schemas, names and CLI mappings fail before publication.

## SRV-OPS-002 — Scoped operation admission

**Requirement.** Operation grants MUST retain explicit plugin permissions and an immutable operation contract snapshot. Discovery and invocation MUST recheck current vault, credential and provider admission. Old read-only grants MUST acquire no plugin authority. Revocation, disable and vault retirement MUST fence before draining accepted work.

### Acceptance details

- A legacy read grant discovers no plugin operations.
- Foreign vaults, missing permissions and revoked grants refuse invocation.

## SRV-OPS-003 — Tasks operation provider

**Requirement.** The Tasks provider MUST delegate reads and supported guarded actions to existing native services. Complete and skip MUST retain ordered journal acceptance and explicit inspection/retry. Transport loss MUST NOT replay mutations. Headless configuration MUST use persisted owner-approved plugin declarations and domain settings.

### Acceptance details

- Stale source versions and editor fences preserve authored content.
- Cold restart and browser detach retain native service authority.

## SRV-OPS-004 — CLI operation help

**Requirement.** The CLI MUST derive plugin commands, arguments and Markdown help from shared descriptors, with bundled Tasks help available offline. Invocation MUST validate current host availability and contract version. Structured arguments MUST remain data. Credential, JSON-output and exit-code policies MUST apply to contributed commands.

### Acceptance details

- Offline help includes argument descriptions and authored examples.
- Invocations use live discovery and never replay a failed write.
- Input files are bounded JSON data; MCP writes retain a caller-known invocation UUID.

## SRV-OPS-005 — Packaged skill sources

**Requirement.** Native hosts MUST discover skill contributions from admitted package files without loading renderer code and expose explicit vault-scoped list, install and update operations. Writes MUST require skill-management permission.

### Acceptance details

- Disabled or unverified plugin sources are excluded.
- The host CLI contribution includes its linked command, credential and protected skill-management references as packaged assets; installed references receive the same full-directory edit protection as SKILL.md.
- Existing read-only grants cannot install or update skills.
- Hosted acceptance MUST exercise the real app commands and CLI against the same isolated vault, including packaged references, local edits and a cold restart.

## SRV-OPS-006 — Protected skill updates

**Requirement.** Updates MUST preserve an entire skill directory when its files differ from its recorded installation. Unmanaged directories MUST remain untouched and skipped results MUST identify the skill and reason.

### Acceptance details

- Edited, added and deleted files preserve the directory.
- Binary edits and additions report local modification and preserve their exact bytes.
- Clean updates install the packaged file set without retaining obsolete resources.

## SRV-OPS-007 — Durable skill installation

**Requirement.** Native serialization MUST own skill writes independently of renderer lifetime. Versioned receipts MUST record source versions, file hashes and pending operations; interrupted operations MUST remain fenced until explicitly resolved.

### Acceptance details

- Cold reopen retains installation ownership and local-edit detection.
- Interrupted writes cannot be mistaken for completed updates or automatically replayed.
