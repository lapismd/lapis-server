# Hosted contract

These requirements moved from the Notes hosted-server chapter. Shared client
behavior and observation streams stay in the Notes specification.

[Native file observation](#srv-hst-021) uses released API `^0.2.8` for user-file
discovery and observation scopes. Installed plugin payloads and registry caches
stay outside the user-file tree; configuration remains owned by its targeted
reload without mirroring external observations back into compatibility files.

## SRV-HST-001 — Native hosted application

<a id="srv-hst-001"></a>

**Requirement.** The hosted application MUST run as the compiled Rust executable, serve the static Web build, and mount admitted native HTTP routes. It MUST NOT use a Node or Deno application runtime, SvelteKit, or SSR.

### Acceptance details

- The runtime process is the compiled `lapis-server` executable.
- Static files and native HTTP routes are served by that process.

## SRV-HST-002 — Authoritative Markdown

<a id="srv-hst-002"></a>

**Requirement.** The server MUST keep Markdown files authoritative under managed `vaults/<id>/content` directories. Per-vault Turso files under `vaults/<id>/state` MUST remain disposable generated state.

### Acceptance details

- Managed vault content stays in the content directory.
- Generated Turso state is not the authority for Markdown.

## SRV-HST-003 — Single-owner control state

<a id="srv-hst-003"></a>

**Requirement.** The native control store MUST own initial owner credentials, sessions, managed vault identities, and archive state. Every vault operation MUST resolve the authenticated owner and admitted vault. Initial operation MUST remain single-user with multiple vaults.

### Acceptance details

- Control data stays outside admitted vaults.
- Vault operations resolve the authenticated owner.

## SRV-HST-004 — Password hashing and sessions

<a id="srv-hst-004"></a>

**Requirement.** Passwords MUST use bounded scrypt parameters and random salts. Sessions MUST use hashed opaque tokens, secure same-site HTTP-only cookies, idle and absolute expiry, strict configured-Origin checks, mutation CSRF validation, throttled login, and revocable subscriptions.

### Acceptance details

- Password hashes use bounded scrypt and do not persist the plaintext password.
- Session cookies and Origin checks follow the configured browser origin.
- Multiple Origin headers are ambiguous and rejected.

## SRV-HST-005 — Typed control plane

<a id="srv-hst-005"></a>

**Requirement.** Native HTTP routes MUST own the typed JSON control plane for system, authentication, vault, filesystem metadata, AppDatabase, and service commands. Browser clients MUST use generated portable contracts and MUST NOT import the native server or obtain SQL or native paths through metadata commands.

### Acceptance details

- `lapis-app-protocol` defines the portable commands and replies.
- Browser clients do not receive raw SQL or native filesystem paths through metadata commands.
- Authenticated terminal output is user-command output and may include native paths.

## SRV-HST-006 — Protocol identity

<a id="srv-hst-006"></a>

**Requirement.** The protocol MUST expose a stable integer major, build id, JSON-compatible schemas, bounded inputs and outputs, and stable application error codes. A stale client MUST receive an explicit protocol mismatch.

### Acceptance details

- The protocol major is a stable integer.
- A mismatched client receives an explicit protocol error.

## SRV-HST-007 — Direct byte routes

<a id="srv-hst-007"></a>

**Requirement.** Streamed file bodies, append, import, export, resources, plugin assets, health, readiness, and static files MUST use direct HTTP routes. A JSON command envelope MUST NOT wrap those byte streams.

### Acceptance details

- Health, readiness, and static files are direct HTTP routes.
- File byte streams are not wrapped in a JSON command envelope.

## SRV-HST-008 — Vault paths

<a id="srv-hst-008"></a>

**Requirement.** Vault paths MUST be normalized relative paths and MUST reject absolute paths, traversal, NULs, and symlink escapes. Replacement writes MUST be atomic, and conditional writes MUST enforce strong ETags.

### Acceptance details

- Absolute paths, traversal, and symlink escapes are rejected.
- Conditional writes enforce strong ETags.

## SRV-HST-009 — Admitted observation

<a id="srv-hst-009"></a>

**Requirement.** Hosted vault changes MUST remain observable to admitted sessions while native services retain sole write authority. Lost observation MUST reconcile current state without replaying mutations. Closing or revoking a session MUST retire its observers.

### Acceptance details

- Admitted sessions can observe committed vault changes.
- Closing a session retires that session's observers.

## SRV-HST-010 — Remote database contract

<a id="srv-hst-010"></a>

**Requirement.** AppDatabase access MUST use the shared remote method contract and a host-owned native Turso worker. Renderers MUST NOT receive raw SQL, database paths, or methods outside that contract.

### Acceptance details

- Renderers use the shared remote method contract.
- Raw SQL and database paths stay on the host.

## SRV-HST-011 — Vault runtime lifetime

<a id="srv-hst-011"></a>

**Requirement.** The server MUST open vault runtimes lazily, serialize mutations per vault, close idle runtimes, and close databases, watches, subscriptions, and imports during ordered shutdown. Initial operation is one native server process.

### Acceptance details

- Mutations for one vault are serialized.
- Ordered shutdown closes databases, watches, subscriptions, and imports.

## SRV-HST-012 — Health and readiness

<a id="srv-hst-012"></a>

**Requirement.** Health MUST report process liveness without authentication or secrets. Readiness MUST remain false until control migrations complete, the managed data root is writable, and the static client is available.

### Acceptance details

- `/healthz` reports liveness without credentials.
- `/readyz` stays unavailable until startup can admit work.

## SRV-HST-013 — Initial host scope

<a id="srv-hst-013"></a>

**Requirement.** The hosted server MUST admit AI only through trusted operator-declared native targets and existing turn, participant, and queue authorities. It MUST NOT offer collaborative editing, arbitrary filesystem roots, invitations, public registration, or multi-instance coordination.

### Acceptance details

- Native agent targets come from operator configuration.
- Missing capabilities are reported instead of being silently enabled.

## SRV-HST-014 — Plugin install route

<a id="srv-hst-014"></a>

**Requirement.** Hosted plugin installs MUST download signed first-party release bundles through an authenticated same-origin Fetch route. The server MUST restrict outbound sources, bound response bytes, and forward no session credentials upstream.

### Acceptance details

- Plugin downloads require an authenticated editor request.
- The server does not forward session credentials to the upstream source.

## SRV-HST-015 — Static plugin host

<a id="srv-hst-015"></a>

**Requirement.** The server static profile MUST serve the fixed Web plugin-host entries referenced by its import map, including `@lapis-notes/api/workspace-host`. Installed History modules MUST resolve those entries through that host graph before activation.

### Acceptance details

- The static profile includes the workspace-host entry.
- History activation uses that same host module graph.

## SRV-HST-016 — Delivery without Deno

<a id="srv-hst-016"></a>

**Requirement.** Server builds, tests, executable output, and production containers MUST run without Deno. Runtime containers MUST use a non-root user, read-only application files, explicit writable storage for profile data and optional Mod cache, and mounted bootstrap secrets.

### Acceptance details

- The runtime image does not include Deno.
- The container runs as a non-root user with read-only application files and writable `/data` and an optional independent Mod cache.

## SRV-HST-017 — Direct native services

<a id="srv-hst-017"></a>

**Requirement.** The Rust host MUST invoke shared native services directly rather than supervise a Node companion. Shutdown MUST drain service-owned work before success, preserve unresolved editor recovery, and distinguish forced termination from graceful completion.

### Acceptance details

- Native services run in the Rust host.
- Graceful shutdown drains owned work before reporting success.

## SRV-HST-019 — No release registry

<a id="srv-hst-019"></a>

**Requirement.** The hosted server MUST NOT become a registry database, relay trust root, curator, or proxy for signed release evidence. A server-mode Web client MAY read its configured plugin source directly.

### Acceptance details

- The server does not persist a plugin registry or proxy signed release evidence.
- The Notes client requirement for that direct source stays in the Notes specification.

## SRV-HST-018 — History, Tasks, and Repository admission

<a id="srv-hst-018"></a>

**Requirement.** Hosted History, Tasks, Repository, backup, and original-source commands MUST use the authenticated vault client and its physical source and generation checks. Requests MUST NOT supply a filesystem root, profile, or caller scope, and revoked or foreign clients MUST lose access.

### Acceptance details

- Those commands reuse the authenticated vault attachment.
- A revoked or foreign client cannot supply a filesystem root or caller scope.
- Bounded History reads may return typed `409`/`CONFLICT`/`history: busy` backpressure.
- Retained acceptance exercises File History Refresh without replaying mutations and rejects unexpected native, admission, restore and unknown-write errors.

## SRV-HST-020 — Authenticated vault terminals

<a id="srv-hst-020"></a>

**Requirement.** Terminal commands and bounded output streams MUST authenticate the owning vault client. The Deno-free native PTY engine MUST use the admitted physical vault as cwd authority. Detach, expiry, logout, successful vault close and shutdown MUST stop owned shells, while dirty-editor close refusal MUST preserve them. The bundled UI MUST require trust and optional-plugin admission.

### Acceptance details

- Create accepts relative cwd and dimensions, never roots or arbitrary shells.
- Inputs, session counts, replay and observers are bounded; output revalidates authentication and has a finite response lifetime.
- Browser input writes preserve per-session order across pending HTTP requests.
- Plugin disable and trust revocation close sessions; output may contain native paths from user commands.

## SRV-HST-021 — Native file-change observation

<a id="srv-hst-021"></a>

**Requirement.** Authenticated clients MAY observe vault-relative file changes through GET `/api/v3/clients/{client}/file-events`. Core MUST own one native watcher per admitted vault, fence retired clients and physical roots, bound replay and frames, signal uncertainty explicitly, and preserve graceful teardown. Browser adapters MUST reuse NativeWatchAdapter, avoid recursive HTTP polling, and recover missed changes before projecting a complete tree.

### Acceptance details

- Scope checks cover owner, vault, generation, origin and expiry throughout a bounded response lease; events contain relative paths without file bytes or credentials.
- Bounded cursors replay retained events; gaps, overflow and watcher errors require reconciliation rather than silently dropping changes.
- The user-file observer excludes app-state trees; the configuration owner retains a targeted file reload, while external user writes, rename, deletion, reconnect and teardown remain covered.
- Cold enabled-plugin and idle tests supplement the unchanged 600-dormant-assets startup budget and exercise the real hosted transport.
