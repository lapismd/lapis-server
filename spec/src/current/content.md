# Content automation

Shared content contracts, native policy and adapters belong to this repository.
Host mounts and plugin settings remain separately qualified in the
[owning backlog](../backlog/content-automation.md).

## SRV-SRV-005 — Portable content automation contract

<a id="srv-srv-005"></a>

**Requirement.** `lapis-app-protocol::content` MUST own transport-independent
content requests, replies, operation descriptors and input validation. Content
API major 1 MUST remain separate from browser attachment protocol major 3.
The initial operation inventory MUST cover capabilities, admitted vault and
note listing, note reads, absent-only creation and conditional replacement.
Contract inventory MUST NOT advertise runtime support or confer authority.

Requests carry a nonnil correlation UUID; mutations reuse it as their future
receipt identity. DTO validation provides no persistence or duplicate-execution
guarantee. Adapters bound transport decoding and call
`ContentRequest::validate` before admission. Limits are 1 MiB UTF-8 text,
4096-byte paths/printable-ASCII cursors, 256-byte printable-ASCII versions and
1–100 items per page. Empty, wildcard and weak versions are refused.

Paths use slash-separated relative components. Empty/dot/traversal components,
backslashes, colons, controls, trailing dots/spaces and case-insensitive `.lapis`,
`.git` and `.jj` components are refused. Physical/symlink checks, grant revocation,
editor fences and serialized publication remain Core/host duties.

Listings describe immediate children; omitted directory selects the admitted
vault root. Cursor validity and result bounds remain service duties. Text reads
include a file version; committed writes do not assert indexed-search freshness.
Attachments remain on direct byte transports. Runtime discovery filters qualified
operations through current admission. This contract mounts no endpoint.

### Acceptance details

- Strict DTOs reject fabricated root/caller/generation/scope authority and unknown fields.
- Replace requires a strong version; create has no overwrite switch; semantic validation enforces the documented bounds and paths.
- Replies retain correlation and distinguish pre-dispatch refusal, committed effects and unknown effects on errors.
- Operation descriptors declare scope/effects without enabling or authorizing runtime capabilities.

## SRV-SRV-006 — Generated protocol drift gate

<a id="srv-srv-006"></a>

**Requirement.** This repository MUST generate and check its committed Rust
protocol JSON Schema and TypeScript bindings. `pnpm protocol:check` MUST fail
without rewriting files when either artifact is stale, and MUST type-check
representative clients. CI MUST run the gate for ordinary pull requests.

Both browser/native and additive content contracts are exported. Notes owns
adoption into its public API package; generation never edits sibling repositories
or publishes packages.

### Acceptance details

- `pnpm protocol:generate` writes only this repository's protocol artifacts.
- `pnpm protocol:check` rejects stale artifacts and checks strict TypeScript clients.
- Rust fixtures retain wire/schema agreement, required preconditions and semantic bounds.

## SRV-SRV-007 — Scoped native content reads

<a id="srv-srv-007"></a>

**Requirement.** App Core content sessions MUST execute reads only through
host-supplied dedicated admitted vault clients. They MUST support explicit
revocation, monotonic expiry and bounded authoritative reads/listings without
granting writes, new filesystem roots or external credential authority.

`ContentSession::read_only` consumes up to 32 distinct vault attachments with
nonempty display names of at most 256 UTF-8 bytes and a positive lifetime of at
most 24 hours. It accepts no caller-selected path or generation. Hosts own
authentication, consent, token verification, session-count limits and idle/expired
session cleanup. Dropping or revoking a session retires its dedicated clients.

Capabilities advertise only discovery, vault listing, note listing and note
read. Note reads return saved UTF-8 content and its existing SHA-256 version,
with a 1 MiB bounded read. Listings include only immediate regular `.md` and
`.markdown` children, case-insensitively, under the existing 2,000 directory-entry
budget. Unaddressable public paths and symlinks are excluded. Cursors retain
the query, session and listing fingerprint, covering ordered paths/sizes or
vault identities/names. The latest 128 distinct continuations are retained;
evicted, foreign or changed-listing cursors return an explicit conflict and
require a first-page refresh. Content changes with identical size do not change
the listing's path/size snapshot; a fresh note read obtains the current version.

Revocation fences new calls before waiting for admitted readers to finish.
Execution rechecks revocation/expiry after session and vault lock waits, and
revalidates the client and physical root. An already admitted read may complete
while revocation drains; the response grants no subsequent access. Native reads
use the vault lock, so close waits for active reads through existing lifetime
ownership. Async adapters must run blocking dispatch/revocation on native workers.
Errors are sanitized and report no mutation effects. No network route is mounted
by this native-library slice; host and transport qualification remain pending.

### Acceptance details

- Read grants reject foreign vaults and all content mutations and advertise only the four implemented read operations.
- Revocation fences a reader waiting on the vault lock and drains its original attachment; expired or closed-vault access fails.
- Pagination rejects cross-session, cross-query, evicted and changed-snapshot cursors without skipping results silently.
- Reads reject unsafe paths, symlink escapes, replaced physical roots, invalid UTF-8 and content beyond 1 MiB.

## SRV-SRV-008 — Ephemeral content test grants

<a id="srv-srv-008"></a>

**Requirement.** App Core MUST offer a host-owned, bounded registry of read-only
test credentials over SRV-SRV-007 sessions. Only a trusted native host may issue,
revoke, expire or shut down grants; request data MUST NOT select authority.
The registry MUST retain only SHA-256 token verifiers and admitted sessions,
never bearer plaintext. Credentials MUST contain 256 bits of OS randomness and
have a distinct `lc1_` prefix. Issued tokens require explicit secret access;
Debug output MUST redact them and no serialization implementation is provided.

At most 128 grants, including revoking grants until their drain finishes, may
occupy one registry. Session vault/name/lifetime limits still apply. Invalid,
foreign, expired or revoked credentials return the same unauthenticated error
before content validation. Verification uses constant-time digest comparison.
Valid credentials dispatch only through their original session, preserving
scope, cursor isolation, lock-wait revalidation and mutation refusal.

Revocation and shutdown fence sessions while holding the registry lock, then
drain outside that lock. Independent grants remain usable during another grant's
drain. Shutdown fences every grant before draining any and permanently refuses
issuance. Hosts must call blocking cleanup outside async executors and schedule
`prune_expired`; elapsed grants cannot execute even before cleanup. Successful
cleanup releases dedicated attachments and capacity. Final registry drop also
retires attachments. Restart creates a fresh registry and accepts no old token.

These credentials qualify controlled native tests only. No hosted route,
persistent grant, token rotation, owner consent, OS credential custody or OAuth
integration is implemented by this increment; those gates remain in the backlog.

### Acceptance details

- Issued credentials expose only their admitted read operations and reject foreign vaults, malformed or foreign credentials and mutations.
- The registry stores verifiers, redacts issued-token diagnostics and refuses issuance at its 128-grant limit until drain releases capacity.
- Revocation fences native-lock waiters while independent grants remain usable; expiry cleanup releases attachments without extending authority.
- Shutdown fences all queued readers before draining, rejects later issuance and releases attachments; fresh registries reject previous tokens.

## SRV-SRV-009 — Shared content HTTP reads

<a id="srv-srv-009"></a>

**Requirement.** `lapis-content-http` MUST supply one explicitly mounted read-only
router for Server and Desktop integrations using SRV-SRV-008 test grants.
It MUST dispatch through Core, with no independent filesystem access or
host-bootstrap dependency. Adding the crate MUST NOT enable a production host
listener, consent flow or persistent credential.

The router exposes GET `/api/content/v1/capabilities`, `/vaults`,
`/vaults/{vault_id}/notes` and `/vaults/{vault_id}/note`. Those suffixes all live
under `/api/content/v1`. Lists accept optional `limit` (default 50) and `cursor`;
note lists also accept `directory`. Note reads require `path`. Duplicate/unknown
query fields, malformed percent/UTF-8 encoding and Core-invalid input are
refused. URI length is limited to 16 KiB and aggregate headers to 8 KiB.

Clients MUST supply one bearer credential and `x-lapis-content-api: 1`.
Credentials are checked before query/version validation and rechecked at native
dispatch. Cookies and the owner-wide server token confer no content authority.
The host configures an exact HTTPS origin, or HTTP on loopback only. Requests
MUST match its Host and any supplied Origin; duplicate authority/authentication
headers and conflicting absolute request targets are refused. Forwarded headers
do not override this policy. No CORS permission or grant-management route exists.

`x-lapis-request-id` MAY supply a nonnil correlation UUID; omission creates one,
and invalid values fail. Replies carry that ID and API major in headers. Control
results/errors use `ContentReply`; note content uses direct UTF-8 bytes and a
strong quoted ETag. Every adapter response is `no-store` and `nosniff`. Errors
use sanitized messages: invalid input 400, credentials 401, scope/origin 403,
missing content 404, protocol/conflict 409, size 413 and busy/unavailable 503.
Unsupported methods return 405. GET bodies are refused; attachments and mutations
remain unavailable. SRV-HST-007 direct-byte transport remains unchanged.

At most 64 native jobs may run per router; excess work fails busy without an
unbounded queue. Blocking credential/dispatch work runs outside async executors.
Cancellation MUST retain its permit until the native worker finishes. Hosts own
listener/connection limits, TLS, credential cleanup and shutdown/drain orchestration.

### Acceptance details

- Real HTTP tests match Core capability/catalog coverage, vault/note pagination, UTF-8 bytes, strong versions and cursor invalidation.
- Missing, foreign or revoked credentials fail before query validation; Host/Origin, protocol, scope and unimplemented routes cannot extend authority.
- Strict query/path validation, body and size limits retain typed sanitized errors with correlation and non-cacheable responses.
- Native admission rejects saturation and retains capacity after caller cancellation until the worker drains.

## SRV-SRV-010 — Initial content CLI reads

<a id="srv-srv-010"></a>

**Requirement.** The private `lapis-cli` crate MUST build the `lapis` executable
as a content HTTP client, without opening vault roots or importing Core in its
production dependencies. Initial commands MUST include `capabilities`, `vault
list`, `note list --vault <UUID>` and `note read --vault <UUID> --path <path>`.
Lists accept `--limit` (default 50) and `--cursor`; note lists accept `--directory`.
All commands reuse Protocol validation and the SRV-SRV-009 HTTP contract.

This increment requires an explicit `--target <origin>`; HTTPS is required
except for HTTP loopback. URL userinfo, non-root paths, queries and fragments
are refused. The client MUST NOT follow redirects, use environment proxies,
infer a target/vault from cwd or fall back to local disk. Requests have a bounded
`--timeout-ms` of 1–120000, default 15000, and retain correlation/API identity.
There is no implicit pagination or retry loop.

Credentials MUST enter through `--token-stdin` or `--token-file <path>`, never
a bearer argument. Input is bounded to 4096 printable ASCII token bytes with
an optional final LF/CRLF. Unix files MUST be regular, owned by the effective
user, have no group/other permissions and be opened without following the final
symlink or blocking on a FIFO. Other platforms use stdin pending an equivalent
permission check. Credential values and raw transport errors MUST NOT be printed.

JSON mode emits exactly one Protocol `ContentReply` to stdout, reconstructing
note results from direct bytes and strong ETags. Human note reads preserve exact
saved UTF-8 bytes without adding a newline; other human output is tabular.
Diagnostics and continuation hints use stderr. JSON errors retain stable codes
and effects with local sanitized messages. Command-line syntax errors use the
parser's stderr/help contract. Responses MUST match request/API identity and
result kind, with at most 1 MiB of note bytes or 2 MiB of control JSON.

Exit codes are 0 success, 2 invalid input, 3 unauthenticated, 4 conflict/already
exists, 5 not found, 6 permission denied, 7 busy/unavailable, 8 protocol mismatch,
9 too large, 10 unsupported and 11 unknown effect. Output failure returns 7;
a closed stdout pipe is normal completion.

Source CLI acceptance does not qualify production host mounting, named profiles,
owner login, OS custody, Desktop discovery, writes, completions or
binary/image distribution. Those remain in the content backlog.

### Acceptance details

- The compiled CLI reads real shared HTTP fixtures with catalog/list/cursor parity and byte-identical notes in human and JSON modes.
- Invalid targets, missing credentials, revoked or foreign-vault access and malformed responses produce deterministic errors without exposing credentials or opening vaults.
- Credential files enforce their Unix ownership/permission/type boundary; stdin is bounded and neither path uses token argv values.
- Redirects, protocol/correlation mismatch, oversized replies and timeouts fail without credential forwarding or implicit retries.

## SRV-SRV-011 — Shared MCP reads

<a id="srv-srv-011"></a>

**Requirement.** The private `lapis-content-mcp` crate MUST expose an explicitly
mounted `/mcp` Streamable HTTP adapter over SRV-SRV-008 test grants. The qualified
matrix is official Rust SDK `rmcp = 3.9.0`, protocol `2026-07-28` and its official
Rust client's discovery lifecycle. Legacy HTTP sessions, OAuth
discovery, public client products and production host mounting remain pending.

The adapter MUST authenticate every request before SDK body parsing and recheck
the credential at content dispatch. HTTP and MCP MAY share a `Boundary` clone
to enforce one 64-worker native budget with SRV-SRV-009 Host/Origin policy.
MCP MUST bound active HTTP handlers to 64, bodies to 64 KiB while streaming and
request handling to 15 seconds. Cancellation MUST retain native worker capacity
until blocking work finishes. Hosts own listener limits, cancellation and grant
shutdown/drain. Session IDs MUST NOT confer authority and MUST be rejected.

Discovery MUST advertise only `capabilities`, `list_vaults`, `list_notes` and
`read_note`, with input/output schemas derived from Protocol and advisory read
annotations. Tool dispatch MUST decode strict Protocol arguments and call Core;
typed content failures retain code/effect with `isError`. Note resources use
`lapis://notes/{vault_id}/{path}`, decode the relative path exactly once and use
the same read operation, preserving saved text/version. Resource lists remain
empty; templates and `list_notes` provide discovery without unbounded traversal.
Catalog/resource cache hints MUST be private with zero freshness; HTTP replies
MUST be no-store. Core's scope, path, payload and cursor limits remain in force.
SDK protocol/body/header checks MUST reject missing or mismatched metadata,
unsupported revisions, batches and unimplemented methods before content access.

### Acceptance details

- The official SDK client discovers the four read tools and reads notes/resources through real HTTP with the same bytes and versions as the HTTP adapter.
- Native scope and cursor isolation, strict arguments, unsafe paths and oversized requests are refused without enabling writes or exposing credentials.
- Revocation between HTTP preflight and body completion prevents MCP dispatch; saturated and cancelled requests release transport capacity while native workers retain their drain guarantees.
- Missing, conflicting or legacy protocol metadata, foreign Host/Origin and session headers fail; cancellation and slow bodies are bounded.

## SRV-SRV-012 — CLI MCP stdio bridge

<a id="srv-srv-012"></a>

**Requirement.** `lapis mcp serve --stdio` MUST bridge to the explicit host's
`/mcp` endpoint without opening native vaults. It MUST use a protected token file;
`--token-stdin` and `--json` are invalid because stdin/stdout carry MCP frames.
The source matrix is `rmcp 3.9.0`, upstream HTTP `2026-07-28` discovery and stdio
`2026-07-28` discovery or `2025-11-25` initialization. Non-Unix credential custody,
public client products, production host mounting and distribution remain pending.
Tools/resources and their schemas MUST come from the host. The bridge MUST strip
caller transport metadata, disable upstream response caching, retain private/zero
freshness hints and use one upstream call without interactive continuation replay.
It MUST preserve scoped results, sanitize transport errors, refuse redirects and
proxies, and bound each upstream request by `--timeout-ms` and 64 active forwards.
Stdio lines MUST be bounded to 64 KiB; oversized lines are discarded before the
next request. EOF or failed stdout MUST terminate/drain async transport services
without waiting indefinitely for Tokio's blocking stdin reader.

### Acceptance details

- Official SDK clients negotiate both qualified stdio revisions and read the shared host's tool/resource catalogs and exact saved notes through the compiled CLI.
- Foreign vaults, revoked grants, malformed replies and startup failures retain safe errors without serving cached data or exposing bearer credentials.
- Timeouts and redirects do not replay or forward credentials, and subsequent independent calls remain usable.
- Protocol-only stdout, oversized-line recovery, EOF and closed-stdout shutdown pass subprocess acceptance alongside the existing CLI reads.

## SRV-SRV-013 — Shared host access controls

<a id="srv-srv-013"></a>

**Requirement.** Core MUST provide an explicitly disabled grant registry for
production host startup, reversible enable/disable controls and owner-only
credential summaries. Hosts MUST authenticate owner control independently from
content bearer access and keep listener/persistence policy outside Core. This
source slice MUST NOT claim either host mount or plugin settings qualification.

Disabling MUST fence every grant before draining any accepted native read;
re-enabling MUST NOT revive old tokens or an issuance started before disable.
Enable, disable and shutdown MUST serialize; shutdown is permanent. Admission
MUST recheck the current enable epoch before publishing a credential. Registry
locks MUST NOT span native attachment validation, reads or drain.

Named issuance MUST validate a nonempty trimmed display label of at most
128 UTF-8 bytes, reject control characters and consume 1–32 dedicated
host-admitted clients into the existing read-only grant.
Summaries MUST expose only the grant UUID, label, admitted vault IDs, read scope,
active flag and bounded remaining lifetime; never tokens or verifiers. They
MUST derive expiry from the original monotonic session deadline and remain
bounded by the existing 128-grant capacity. Failed drains remain fenced and
retain capacity until cleanup succeeds.

Vault retirement MUST fence every grant containing that vault before draining
any of them, leaving unrelated grants usable. The host MUST serialize native
vault admission against retirement and fence pending issuance before closing
the runtime. Whole-grant revocation cannot silently broaden or replace scope.
Credentials remain process-local and require a new issuance after restart.

### Acceptance details

- A disabled registry refuses issuance and bearer access; enable permits scoped read-only credentials; disable drains them and re-enable never restores old authority.
- Owner summaries contain validated labels, exact admitted scope and monotonic expiry, without secrets; expired grants still refuse calls before cleanup.
- Concurrent tests hold native readers/issuance, proving all-grant fencing before drain and stale-admission refusal across disable/re-enable or vault retirement.
- Vault retirement preserves unrelated credentials; permanent shutdown cannot re-enable and retains the existing capacity, cleanup and cancellation guarantees.

## SRV-SRV-014 — Hosted content access and plugin settings

<a id="srv-srv-014"></a>

**Requirement.** The hosted process MUST optionally serve the shared content
HTTP API at `/api/content/v1` and stateless MCP at `/mcp`, using one native worker
budget and scoped Core credentials. Access MUST start disabled for a new profile.
The public origin MUST satisfy the shared HTTPS/loopback policy; an incompatible
origin leaves content unavailable without breaking existing owner routes.

Owner controls under `/api/v3/content-access` MUST use existing owner/session,
protocol and CSRF checks; content tokens MUST NOT authorize those controls.
Controls MUST bound request bodies, admission and native work, retain accepted
work after disconnect, and drain before host shutdown. Vault close/archive MUST
retire affected whole grants while excluding concurrent native admission.

The control database MUST persist only the enable preference, never content
tokens. Grants remain process-local, read-only, named, explicitly scoped to
1–20 active managed/configured vault IDs, and valid for at most 24 hours. Only
issuance reveals the secret; list/status never do. Disable MUST fence/drain even
if saving the preference fails. Enabling MUST save consent before admitting
credentials. Token-only installations MAY use process-local preferences.

The hosted Notes workspace MUST register a Content API and MCP plugin settings
section under **Core plugins** using the public declarative settings API and
`navigationGroupId: "core-plugins"`. It MUST show enablement,
endpoint availability/addresses, selected vaults, read permission, expiry,
creation and revocation. One-time tokens MUST stay in transient memory, clear on
teardown/dismissal, and never enter browser/vault settings storage. Desktop
integration, OAuth and write operations remain independently gated.

### Acceptance details

- Real HTTP and subprocess tests cover disabled startup, owner authorization/CSRF, enable, scoped HTTP/MCP reads, foreign-vault refusal, revocation and disable/re-enable.
- Cold restart preserves the enable preference but refuses old tokens; vault close/archive and shutdown retire native grants, while unsupported origins preserve owner access.
- Accepted control work and bounded native permits survive response cancellation; shutdown drains controls and content readers without reviving authority after a failed native leave.
- The real hosted plugin settings screen creates/revokes credentials and disables access; production catalog interaction/axe and cold built-server acceptance pass without browser credential persistence.
