# Content API, MCP and CLI

Disposition: portable contracts, scoped native reads and ephemeral test credentials implemented under SRV-SRV-005/006/007/008;
no new endpoint, executable command or host capability is enabled. The end goal is one content API
available through the hosted server and Lapis Desktop, an MCP endpoint in both,
and a first-party `lapis` CLI for interactive use, scripts and agents.

## Existing foundations and gaps

The first implementation increment adds strict content DTOs, request validation,
an operation/scope/effect catalog and generated bindings with a CI drift gate.
The next increment adds a read-only Core session with admitted native clients,
revocation/expiry, bounded reads and snapshot-bound pagination. A bounded native
test-grant registry now issues random read-only credentials, retains only
verifiers and fences/drains revocation, expiry and shutdown. Persistent grants,
rotation, owner consent, write dispatch, durable operation receipts, HTTP/CLI/MCP adapters
and Desktop intake remain unimplemented. OAuth/SDK selection and consumer contracts remain part of
the first contract slice below; do not mark that whole slice or either host done.

The server already exposes authenticated `/api/v3` commands and direct file-byte
routes. `LAPIS_SERVER_TOKEN` admits a single owner-level bearer principal;
browser sessions separately enforce Origin, CSRF and session expiry. This is a
foundation, not a scoped external-application credential model. See
[authentication](../../../crates/lapis-server/src/http/auth.rs) and
[identity](../../../crates/lapis-server/src/http/identity.rs).

[App Protocol](../../../crates/lapis-app-protocol/src/lib.rs) already supplies
versioned DTOs, schemas, errors and conditional versions.
[File commands](../../../crates/lapis-app-protocol/src/files.rs) cover low-level
IO. [App Core](../../../crates/lapis-app-core/src/services/mod.rs) owns admitted
clients, writer serialization, editor fences, metadata, Tasks, daily policy and
History integration. Reuse these owners rather than introducing another store.

Desktop consumes Core/Protocol at a pinned server Git revision. Its existing
[application-tool bridge](https://github.com/lapismd/lapis-desktop/blob/main/spec/src/current/ai-tools.md)
already supports private MCP transports, but executes through an AppToolHost
bound to a worker/window. A general content endpoint needs native service
execution independent of a mounted renderer. Existing agent approvals and
unknown-effect handling cannot be bypassed by routing through the new endpoint.

Source availability does not qualify a capability in either host. Existing
[native-service](native-services.md) and [hosted qualification](qualification.md)
gaps, including metadata/backup teardown, remain open.

## Reference findings

Research reviewed on 2026-11-09; upstream default branches can change.

- [Obsidian CLI](https://obsidian.md/help/cli) provides the functional reference:
  vault/file selection, note creation and editing, search, properties, tags,
  links, daily notes, templates, tasks and history. Its CLI controls the running
  app and includes desktop-only commands. Borrow its useful command families;
  Lapis also needs a remote server target and deterministic script behavior.
- GitButler's [API layer](https://github.com/gitbutlerapp/gitbutler/blob/master/crates/but-api/src/lib.rs)
  coordinates shared application operations and locking. Its
  [API macro](https://github.com/gitbutlerapp/gitbutler/blob/master/crates/but-api-macros/src/lib.rs)
  derives Tauri/JSON wrappers around Rust functions. Adopt shared handlers and
  checked adapter coverage, without requiring a new procedural-macro framework.
  Its [CLI](https://docs.gitbutler.com/cli-overview) demonstrates a first-class
  scripting surface alongside the application.
- Block's [Buzz external-app authorization](https://github.com/block/buzz-app/blob/main/docs/agents.md#external-app-authorization)
  keeps keys in the app, offers consent and revocation, and gives external
  clients the same admitted tool handlers. Borrow those lifecycle boundaries.
  Lapis should grant explicit vault/read/write scopes rather than copy Buzz's
  broad agent scope or expose its unrelated shell/agent management features.
- The current [MCP transport specification](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports)
  distinguishes modern per-request metadata from older initialized sessions.
  Select and test a protocol/SDK compatibility matrix; copying an older private
  MCP bridge is not interoperability evidence. Follow the
  [HTTP authorization specification](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization)
  for OAuth discovery and resource-bound tokens.

## Shared architecture and ownership

The proposed call path is CLI/HTTP/MCP adapter → admitted content operation →
existing Core services → authoritative Markdown and disposable projections.
The desktop UI reaches those same services through its native adapter. The CLI
never opens a vault as a second writer behind a running host.

| Owner                                        | Planned responsibility                                                                                                                                                      |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Server-owned `lapis-app-protocol`            | Content DTOs, schemas, stable operation IDs, errors, capability descriptors and portable results. Extend the existing crate.                                                |
| Server-owned `lapis-app-core`                | Content orchestration, authorization context, shared operation catalog, conditional mutations and existing service admission/drain. No HTTP, MCP or Tauri dependencies.     |
| Proposed `lapis-content-http`                | Reusable content HTTP router and mapping, with a host-supplied authentication/admission port; mount in Server and Desktop. No server bootstrap/control database dependency. |
| Proposed `lapis-content-mcp`                 | One shared tool/resource catalog and handlers calling the content service; protocol transport wiring remains outside Core.                                                  |
| Proposed `lapis-cli` (`lapis` binary)        | Parsing, target selection, credentials, content HTTP client, output and a stdio MCP bridge. No independent content algorithms or native vault writes.                       |
| `lapis-server`                               | Remote listening, owner/grant persistence, managed-vault admission, TLS/proxy configuration and hosted acceptance.                                                          |
| `lapis-desktop`                              | Local listening/discovery, consent, credential custody, selected vault/window admission, UI actions, installation and native acceptance.                                    |
| `lapis-notes` and owning plugin repositories | Public TypeScript App/API contracts and generated bindings; reusable Markdown, Search, Tasks and template semantics in their existing owners.                               |

Crate names are proposed. Confirm dependency direction in the first slice and
update SRV-SRV-004 when adding workspace members. Share crates through pinned
Git revisions, with internal Cargo paths only. Keep jj-store at the required
revision and lapis-ai-service at `v0.2.0`. This work requires no crate or npm
publication. Any Notes package release remains a separate owning-repo action.

Use a single explicit catalog for each operation's ID, input/output schemas,
required capability/scope, effect classification and handler. Derive discovery,
MCP schemas and contract fixtures from it; verify HTTP routes and CLI mappings
against it. Frontend/CLI ergonomics can differ without duplicating policy.
Do not automatically expose every internal command or plugin callback.

## Functional scope

These are planned Lapis capabilities, not a claim of Obsidian compatibility.
Every supported content operation should have HTTP, CLI and MCP coverage;
desktop UI operations are separately advertised.

| Delivery                | Operations                                                                                                                                                | Main dependency                                                           |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| First end-to-end slice  | Capabilities; admitted vault list; note list/read/create/conditional replace                                                                              | Existing client admission, file IO and versions                           |
| Initial content release | Append/prepend; copy/move/rename; recoverable delete/list-trash/restore; folders; attachment upload/download; bounded text search with context            | Shared transforms, recovery storage, query service and byte routes        |
| Knowledge workflows     | Property get/set/remove, tags/aliases, outline, backlinks/outgoing/unresolved links; daily read/ensure/append; template list/read/apply; task list/update | Qualified native metadata/Search/Tasks/daily services and shared settings |
| History workflows       | Version list/read/diff; explicitly guarded restore                                                                                                        | Qualified native repository/history owner and recovery checkpoints        |
| Desktop conveniences    | Open/reveal note, active note, selection, allowlisted registered commands                                                                                 | Explicit window binding and live UI capability                            |
| Deferred                | CLI TUI, bulk transactions, semantic search/model setup, plugin installation/management, sync administration, publishing and arbitrary evaluation         | Separate feature contracts and qualification                              |

Daily dates/timezones and template rules use shared settings, not CLI-local
defaults. Reads report index freshness and unavailable capabilities. Task edits
use existing guarded Task operations rather than raw checkbox substitutions.
Property transforms preserve unrelated Markdown/YAML through the owning parser.
Attachments use bounded streams, not base64 payloads in MCP tool calls.

Rename initially moves the addressed note only and documents that behavior.
Automatic link rewriting is a later explicit capability requiring a bounded
affected-file preview, versions, editor guards and partial-failure recovery.
Recoverable deletion must work in both hosts; do not equate hosted deletion
with an OS trash API. Start with files and empty directories; recursive and bulk
deletion remain deferred. History restore creates a guarded new content write.

## Contract and mutation rules

- Require an explicit admitted vault ID and normalized relative path in the
  service contract. CLI names resolve to IDs once; ambiguous names fail with
  candidates. Native roots, client identities and generations come from the
  host, never request-supplied authority. Reserve internal `.lapis`/state paths.
- Reads return content plus a strong opaque version. Create means absent-only;
  replace/append/prepend/move/delete/restore require the expected source state
  and destination collision policy. HTTP maps these to conditional headers or
  typed preconditions; MCP and CLI retain the same semantics.
- Reuse the per-vault writer and editing-intent fences. A clean open editor may
  still block an external mutation. Return a typed conflict/busy result, with
  explicit UI recovery where available. No external force-save/discard route.
- Put authorization, current vault generation and scope revalidation at the
  mutation admission boundary, including after waits. A grant's client cannot
  operate on another grant's attachment, receipt, stream or recovery handle.
- Give mutations an operation ID. Store bounded, credential-scoped receipts
  outside vault content, binding the ID to arguments/preconditions. Duplicate
  requests reconcile an existing result; changed arguments fail. Define crash
  windows explicitly: interrupted publication may be unknown, never assumed
  rolled back or blindly replayed. Set receipt retention/expiry before release.
- Return stable structured errors and effect status: refused before dispatch,
  committed, or outcome unknown. Map HTTP status, MCP tool error and CLI exit
  codes consistently. Cancellation/disconnect never implies rollback; accepted
  work is drained by its owner and can be inspected through operation status.
- Preserve authoritative files, atomic publication, path/symlink protections,
  bounded payloads and normalized encoding/newlines. List/search cursors are
  opaque and scoped; detect invalidation instead of silently skipping results.
- Successful mutations feed existing observation/indexing. Report committed
  file version separately from index freshness; a successful write need not mean
  immediate indexed search visibility. Reconnection refreshes state without
  repeating mutations. Audit principal/operation/outcome, excluding note bodies,
  tokens, native paths and raw queries by default.

## HTTP, authorization and MCP

Proposed public routes live under `/api/content/v1`, alongside the existing
browser `/api/v3` contract. Confirm this separation in the contract slice;
content API major and internal attachment protocol major are distinct. Include
capabilities, vault discovery, notes, search, operation status and direct byte
routes. Preserve SRV-HST-007: file bodies, uploads/downloads and append bodies
remain direct streams. Reuse byte service implementations without exposing
browser attachment lifecycle as the external user's programming model.

External grants start read-only and name admitted vaults. Separately grant
content writes, destructive/recovery operations and desktop UI actions. Tokens
expire, rotate and revoke; store only token verifiers at the server and keep
client credentials in OS custody or explicitly protected headless configuration.
Do not use the bootstrap password or existing owner-wide token as the public
integration credential. Preserve existing cookie/Origin/CSRF behavior and do not
silently extend scoped grants to legacy administrative routes.

Desktop starts an opt-in loopback listener with an ephemeral port and private
discovery metadata. A named profile resolves to a validated local instance;
discovery is not authorization. Use owner consent and a single-use exchange
bound to the requesting client (state/PKCE), then a scoped grant. Validate Host,
Origin and callback targets; never put bearer tokens in deep links or argv.
Define secure discovery/custody independently for macOS, Linux and Windows.
Revocation or vault closure fences new calls and drains admitted work. Durable
approval may survive app restart; live connection authority never does.

Both hosts mount `/mcp` with the same content tool implementation. Start with
small task-oriented tools such as `list_vaults`, `list_notes`, `read_note`,
`create_note`, `replace_note` and `search_notes`; add later families with their
service slice. Expose bounded read-only note resources using opaque vault IDs
and safe relative-path encoding. Resources enforce the same scopes as tools.
Tool annotations describe effects but never authorize them. Prompts are optional
and outside the first release.

Use the official Rust SDK if the compatibility spike proves it fits; pin its
version and explicitly list supported MCP revisions/client versions. Target
Streamable HTTP and provide `lapis mcp serve --stdio` for subprocess clients,
delegating to the selected host rather than opening another vault runtime.
Test modern per-request and any selected legacy initialization paths separately;
do not confuse an MCP transport session with a Core client or grant lifetime.
For remote integrations, implement OAuth protected-resource discovery and a
qualified authorization-server integration with PKCE/resource-bound tokens.
Static scoped tokens may serve initial controlled CLI tests, but do not qualify
general remote MCP connection flows. Keep the existing private agent bridge
until its original approval/cancellation/provider cases pass through shared code.

## CLI behavior and distribution

Use conventional subcommands and flags; Obsidian syntax compatibility is not
required. Illustrative interface, to finalize in the contract slice:

```text
lapis auth login --target work
lapis vault list --target work --json
lapis note read --target work --vault personal --path Inbox/idea.md --json
lapis note create --vault personal --path Inbox/new.md --stdin
lapis note append --vault personal --path Inbox/idea.md --if-version <version> --stdin
lapis search --vault personal --query "project atlas" --json
lapis daily append --vault personal --date 2026-11-09 --stdin
lapis task list --vault personal --json
lapis operation status <operation-id> --json
lapis mcp serve --target desktop --stdio
```

Target precedence: explicit flag, named profile/environment, saved default;
otherwise fail with setup guidance. Support both a remote URL and Desktop.
Never silently switch to another host or infer a writable native root from cwd.
An active-file shorthand is explicit and desktop-only, bound to one window;
automation uses explicit vault/path. Desktop may be launched with an explicit
connect/open action; unavailable Desktop never falls back to direct disk IO.

Provide human-readable output, stable `--json`, stdin/file content input, shell
completion, command help, timeouts and stable exit codes. Stdout contains only
the selected data format (or MCP frames in stdio mode); diagnostics use stderr.
Noninteractive writes require all preconditions and never prompt/retry silently.
Implement read-modify-write convenience only with a retained baseline and
conflict reporting. Keep operation IDs available after a lost response.

Ship a Rust binary through release artifacts and optional Desktop installation;
bundle it in the server image for explicit use. The remote target needs no GUI.
Test clean installation and version mismatch on supported OS/architectures;
document CLI/API/MCP compatibility separately from image tags. Release automation
changes, signing and publication are later explicit release work, with no crate
or npm publication from this repository.

## Implementation sequence and exit gates

1. **Freeze contracts and prove reuse.** Inventory existing native operations,
   public Notes contracts and private MCP helpers. Select crates, auth flow,
   protocol/client matrix, budgets, receipt retention, trash semantics and CLI
   grammar. Add owning requirements/verification in Server, Notes and Desktop
   before implementation; protocol-export drift checking is now restored here. Exit:
   schema/catalog fixtures and a dependency review, with host gaps explicit.
2. **Prove one shared vertical slice.** Implement list/read/create/conditional
   replace, scoped test grants and minimal HTTP/MCP/CLI adapters. Run the same
   scenario against hosted Rust and real Desktop admission: create via CLI,
   read via MCP, replace via HTTP, observe in the UI, reject a stale write.
   Exit: shared handler/schema coverage and zero foreign-vault/editor bypasses.
3. **Complete content operations.** Add safe transforms, file management,
   recoverable deletion, streamed attachments, text search and operation
   reconciliation. Exit: identical results/errors across adapters, competing
   writers and interrupted publication/restart tests; no blind retries.
4. **Qualify external connections.** Deliver owner consent/revoke settings,
   Desktop discovery and credential storage, remote OAuth discovery, expiry,
   scope enforcement, stdio bridge and real MCP-client acceptance. Exit:
   packaged Desktop and actual hosted process tests for revocation, stale
   handles, cancellation, closed windows/vaults and shutdown drain.
5. **Expand knowledge workflows.** Add metadata, links, daily/templates and
   Tasks using qualified existing services, then History read/diff/restore.
   Move shared renderer-only policy to its owning engine when required.
   Exit: each capability passes UI/CLI/HTTP/MCP parity and cold recovery before
   discovery advertises it; disabled plugins/indexes remain explicitly absent.
6. **Ship and document.** Complete CLI help/completions/profiles and installation,
   public API reference/examples and MCP client setup. Validate a coherent
   source/dependency/image/binary cohort. Exit: fresh install, upgrades, old-client
   refusal, credential cleanup and supported-platform acceptance receipts.

Slices 2–3 can be an internal preview. The initial external release requires
slice 4 as well as content acceptance; broad automation is not enabled merely
because an endpoint responds. Knowledge and desktop-convenience commands follow
individually when their dependencies qualify. No TUI is needed for the CLI MVP.

## Verification and remaining decisions

Run shared conformance fixtures against Core, HTTP, MCP and CLI, then the actual
hosts. Cover missing/foreign vaults, scopes, revoked/expired grants, path escapes,
duplicate requests, conflicting preconditions, create collisions, malformed and
oversized input, Unicode, stale indexes and capability/version mismatch.
Exercise open clean editors, dirty/detached editors, multi-window competing
writes, rename/trash recovery, cold restart, lost replies, cancellation and
shutdown with work in flight. Preserve failed attempts and unknown outcomes.

Server owns Rust tests/Clippy, HTTP/MCP integration, real hosted UI and container
acceptance. Desktop owns packaged/native and OS discovery/install acceptance;
Notes owns public API/type tests and shared UI contract coverage. Browser mocks
and Core tests cannot qualify those host gates. Existing teardown quarantines
cannot be used to waive these new lifecycle scenarios. Record results in owning
verification rows; this plan contributes no passing product evidence.

Before slice 2, settle whether an existing authorization service can satisfy
remote OAuth or a bounded first-party authorization component is needed; prove
the chosen Rust SDK/client compatibility; locate any renderer-only transform
and settings ownership; define trash retention/restore collision behavior and
operation-receipt crash recovery. Confirm `lapis` executable naming and initial
packaged platforms. These are bounded design decisions, not permission to omit
the CLI, MCP or either host from the end goal.
