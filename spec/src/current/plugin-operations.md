# Plugin operations

Source contracts and qualification remain distinct. Outstanding host and provider
acceptance belongs to the content automation backlog.

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

