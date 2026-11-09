# Hosted server

This book owns the hosted server. Shared Notes product requirements stay in
the Notes specification at <https://lapismd.github.io/lapis-notes/>.
The core crate README routes native ownership and commands here; outstanding
integration and qualification live in the [canonical backlog](../backlog.md).

<!-- Unchanged contract for the participant profile receipt comparison: each delivery is Completed and its outcome is published before the receipts are compared. Validation is `cargo test -p lapis-app-core --test participant_profiles --locked`. -->

## SRV-SRV-001 — Public image

<a id="srv-srv-001"></a>

**Requirement.** The server image MUST be `ghcr.io/lapismd/lapis-server`. It
MUST listen on port `8787`, store the profile on volume `/data`, and report
readiness at `http://127.0.0.1:8787/readyz`. It MUST NOT be
`ghcr.io/lapismd/lapis-notes-ci` or `ghcr.io/lapismd/lapis-ci`.
A push to `main` MUST publish that image tagged with the 7-character
`git rev-parse --short` commit hash. A `v*` git tag MUST publish the same
image with that tag. Published tags MUST provide `linux/amd64` and `linux/arm64`
manifests. The image web stage MUST install this repository with the frozen
lockfile and build the Vite application in `web/`.
The Rust and Web build stages MUST execute on the native build platform; the
Rust stage MUST cross-compile the requested target architecture rather than
compile the ARM64 dependency graph through QEMU.
The runtime image MUST provide Homebrew at `/home/linuxbrew/.linuxbrew` on a
separate persistent volume and MUST include native Codex, Cursor CLI, Claude
Code, and OpenCode executables on both published architectures. Operators MAY
request additional Homebrew formulae through
`LAPIS_SERVER_HOMEBREW_PACKAGES` and casks through
`LAPIS_SERVER_HOMEBREW_CASKS`; formula installation MUST use a published
bottle and startup MUST fail rather than build from source or ignore an
installation error.
`lapis-ai-service` stays the git dependency
`https://github.com/lapismd/lapis-ai` tag `v0.2.0` at version `0.2.0`.

### Acceptance details

- The Dockerfile exposes port 8787, volume `/data`, and the readyz URL, while Cargo.toml pins lapis-ai-service to tag v0.2.0 and version 0.2.0.
- Main primes the ARM64 Homebrew tooling layer with serialized bottle downloads before publishing the 7-character commit hash and both architecture manifests, and a v\* tag publishes that version tag with both manifests.
- The image cross-compiles each target Rust executable and builds `web/` with Vite on the native build platform after a frozen lockfile install.
- The runtime exposes `brew`, `codex`, `cursor-agent`, `claude`, and `opencode` as the non-root user, persists the Homebrew prefix independently from `/data`, and accepts bounded startup package names while requiring bottles for formulae and rejecting invalid or failed requests before starting the server.

## SRV-SRV-002 — jj-store git revision

<a id="srv-srv-002"></a>

**Requirement.** History, vault, and repository crates MUST use the Cargo git
dependency `https://github.com/stevejuma/jj-store` revision
`7a98bd89d2bc77fb6ec41f91186992abb19ba161`. This repository MUST NOT use a
jj-store path, a crate tag, or pull request 22 head
`35356e655765143b29a6c9b1518ef3b91d04837d`.

### Acceptance details

- The core and server manifests pin that jj-store revision.
- The configure script rejects a jj-store path, a crate tag, and the unmerged head.

## SRV-SRV-003 — Initial owner password

<a id="srv-srv-003"></a>

**Requirement.** A non-empty `LAPIS_SERVER_BOOTSTRAP_PASSWORD` MUST supply the initial owner password and MUST win over `LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE`. The password file path MAY be absent when that variable is non-empty. When that variable is unset or empty, the server MUST read the password file. The server MUST NOT log the password, and an existing owner MUST keep the first password.

### Acceptance details

- The selector returns the environment fixture when both sources are present.
- The selector returns the file fixture when the environment value is unset, and the source message contains neither fixture.

## SRV-SRV-004 — Server crates

<a id="srv-srv-004"></a>

**Requirement.** This repository MUST own the `lapis-server`, `lapis-app-core`, `lapis-app-protocol`, `lapis-content-http`, `lapis-content-mcp` and `lapis-cli` crates, and Cargo paths MUST stay internal. Shared Notes product requirements MUST stay in Notes. Retained native projection engines MUST compile by default through `local-sibling-projections` using exact Git commits. Compilation MUST NOT change per-vault native ownership or configuration.

### Acceptance details

- The workspace contains those six crates; HTTP/MCP adapters share Core/Protocol without server bootstrap or Tauri, while the CLI depends on Protocol and uses native owners only in tests.
- The core and server manifests have no path dependency that leaves this repository.
- The Notes hosted-server chapter points at this book instead of restating it.
- Default `local-sibling-projections` builds and tests the retained engines through Git dependencies, never external workspace paths.

## Native engine source intake

Native Markdown/Search algorithms use lapis-plugins commit
`879581d1549187bdbac2d75b40d02e0df46e721a`, Tasks uses
`52270c2aa7838811604a20be0861cdbd04027b42`, and Docs uses
`f772f2bacc20a914cef2f0398f294cefbac4ba3c`. These remain owning-package
engines. App Core retains admission, scheduling, writer fencing and persistence;
host/native/provider/distribution qualification remains separate.

## Backup initialization diagnostic review

Unchanged-contract confirmation for SRV-SRV-004: debug-only, explicitly selected
backup initialization and shutdown diagnostics retain Core ownership,
admission, storage selection, capture ordering and accepted-work drain. Public
errors remain sanitized; records contain only fixed stages/categories,
process/attempt IDs, elapsed time and retained OS error codes where available.
They exclude paths, native error text, credentials and vault content. Release
builds emit none. This observability change neither repairs nor qualifies the
intermittent Desktop failure; its original quarantine and assertions remain
unchanged. Initial coverage is recorded in the
[initialization receipt](../../records/2026-11-02-backup-initialization-diagnostics.md);
the demonstrated shutdown boundary and its extension are recorded in the
[shutdown receipt](../../records/2026-11-03-backup-stop-diagnostics.md).

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
owner login, OS custody, Desktop discovery, writes, MCP stdio, completions or
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
Rust client's discovery lifecycle. Legacy initialized sessions, stdio, OAuth
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
