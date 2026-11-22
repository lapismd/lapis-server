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

## SRV-OPS-008 — Scoped Markdown note writes

**Requirement.** Hosted Content API, MCP and CLI MUST share native operations for absent-only Markdown creation and strong-version replacement under an explicit `notes:write` grant. The native writer MUST enforce the admitted vault, restricted public Markdown paths and existing editor fences.

### Acceptance details

- Create refuses any existing destination and replacement refuses missing, changed or dirty notes.
- MCP approval and Content API credentials grant the registered tools only when the owner selects `notes:write`.
- CLI `notes create` and `notes replace` discover the same descriptors and accept bounded Markdown input and explicit invocation identities.
- Read-only grants and other write scopes confer no note mutation authority.

## SRV-OPS-009 — Scoped note search and frontmatter

**Requirement.** The shared catalog MUST expose `notes.search` and `notes.frontmatter-get`
under `notes:read`, and `notes.frontmatter-patch` under `notes:write`. Issued
operation snapshots retain their original authority. Generated OpenAPI, MCP
discovery and CLI help MUST use the same descriptors.

Search MUST query one admitted vault using the native Markdown index and scope
its ranking to public saved Markdown. It MUST return bounded ranked paths,
titles and snippets with the applied retrieval mode and index status. Keyword
search works independently of embeddings. Semantic and hybrid searches use
the vault's local model and return a typed error if unavailable. Auto may fall
back to keyword search and MUST report that mode.

Frontmatter reads use the saved file and return its version. An atomic patch
sets and removes individual YAML keys using the strong version and native
editor fence. It MUST preserve the note body and unrelated YAML, pass through
existing indexing, and refuse unsafe layouts or stale versions.

### Acceptance details

- The public operation catalog matches Content API, MCP and CLI invocation.
- Semantic retrieval finds relevant Markdown without a distinctive keyword match.
- Saved metadata patches preserve unrelated YAML, body bytes and editor fences.

## Qualification and release intake

Skill package preparation and CLI qualification follow [SRV-OPS-005–007](plugin-operations.md); source checks and consumer adoption remain separate.

[Final skill qualification](../../records/2026-11-14-skills-final-source-acceptance.md)
records the final Core and public-package acceptance separately from release intake.

Plugin-skill verification lives in its [own table](../verification/plugin-skills.md)
so every canonical page retains the existing 32 KiB limit. SUMMARY navigation
and its metadata-only Storybook mirror retain identical chapter order.
[CI regression evidence](../../records/2026-11-14-skills-verification-page-split.md)
retains the failure, cancellation cause and unchanged verification requirements.

Native skill byte preservation and embedding-cache lease release retain the
[exact failures and source qualification](../../records/2026-11-15-skill-bytes-cache-lease.md).
Complete Core/Protocol/CLI qualification passes 651 cases and strict Clippy;
hosted package and Desktop adoption remain separate registry-intake gates.

Packaged CLI documentation qualification includes actual help grammar, native
resource installation and preservation after a nested reference edit.
[Reference evidence](../../records/2026-11-15-cli-skill-references.md) records
the exact failing/passing source regression independently of host delivery.

The API/profile intake retains the exact `pnpm check` baseline: four duplicate-App
type errors with the older registry graph. Temporary acceptance archives are
confined to the documented validation consumer; publishable manifests retain
normal semver and the committed lockfile must resolve registry packages.

Host-browser qualification runs after local Docker workers finish: bridge/network
changes can cancel Chromium imports and in-flight fetches with ERR_NETWORK_CHANGED.
The [environment record](../../records/2026-11-15-hosted-task-recovery-quarantine.md)
retains failed attempts and the withdrawn provisional quarantine. All Task
assertions remain enabled for the stable-network exact/full rerun.
