# Hosted server

SRV-SYNC-020 native leave preparation retains original account ownership until
host commit; refused leave releases only the admission fence.

Shared Core retains SRV-SYNC-019 account actions beyond response-waiter lifetime,
with single-flight admission, passive observations and one native close drain
that fences later work before releasing custody on confirmed success.
Desktop remains responsible for profile/window admission and OS custody.

SRV-SYNC-018 owns the protected native account coordinator, persisted mutation
intent and proof-bound recovery; native source qualification grants no content
authority or application activation.

SRV-SYNC-017 governs native account protected state and original-scope custody.
Fresh owners reserve their public identity before the first custody lookup.
The portable store requires an explicit host custody port; no platform fallback
or application activation is introduced by its source foundation.

Proof-bound native handoff inspection and cancellation follow SRV-SYNC-016.
The shared native account transport follows SRV-SYNC-015; its bounded
handoff/discovery client grants no endpoint or working-vault authority.

This book owns the hosted server. Shared Notes product requirements stay in
the Notes specification at <https://lapismd.github.io/lapis-notes/>.
Android and iOS hosts consume the shared checked vault file operations directly from lapis-app-core. Target-specific dependencies keep desktop services outside the mobile file-operation graph. Mobile hosts own directory admission and write serialization under SRV-SRV-004.

The core crate README routes native ownership and commands here; outstanding
integration and qualification live in the [canonical backlog](../backlog.md).

The [plugin operation contract](plugin-operations.md) extends shared Core and
Protocol with namespaced descriptors and credential-bound dispatch. It does not
alter native engine ownership or the crate dependency direction in SRV-SRV-004.
The compiled note provider delegates to the existing native writer under
[SRV-OPS-008](plugin-operations.md#srv-ops-008--scoped-markdown-note-writes).
Scoped external note search and saved frontmatter operations use the same
registry and versioned Content API route, including MCP and CLI discovery.
MCP discovery and the OAuth challenge list `notes:write` with other registered scopes. An owner can grant it for selected vaults through OAuth or Content API and MCP settings.
The compiled Tasks adapter incorporates its owning repository's current-main
intake at a pinned Git revision. Workspace patches bind Core/Protocol to this host's
internal workspace instances without introducing an external Cargo path.

The disabled Sync object, operation-journal, dependency-ordered event/HEAD and
stable-arrival repositories are internal `lapis-app-core` modules under
SRV-SRV-004 and [SRV-SYNC-004–007](sync.md). They add no crate, external path,
renderer scheduler, host route or default service. Algorithm ownership remains
in Plugins and host admission remains unimplemented.

The disabled Sync account-connection store is an internal `lapis-server`
control module under SRV-SRV-004 and [SRV-SYNC-002](sync.md). It retains only
S256 challenges and bearer hashes, binds approval to an active owner session,
and grants no vault attachment, endpoint enrollment or content authority.
Its bounded native initiation/redemption routes and owner-cookie decision route
are governed by SRV-SYNC-002. Initiation returns a consent URL under the configured
public origin. The static hosted renderer reviews that request before an explicit
owner decision under SRV-SYNC-014. Desktop integration remains open.

<!-- Unchanged contract for the participant profile receipt comparison: each delivery is Completed and its outcome is published before the receipts are compared. Validation is `cargo test -p lapis-app-core --test participant_profiles --locked`. -->

## SRV-SRV-001 — Public image

<a id="srv-srv-001"></a>

**Requirement.** The server image MUST be `ghcr.io/lapismd/lapis-server`. It
MUST listen on port `8787`, store the profile on volume `/data`, and report
readiness at `http://127.0.0.1:8787/readyz`. It MUST NOT be
`ghcr.io/lapismd/lapis-notes-ci` or `ghcr.io/lapismd/lapis-ci`.
A push to `main` MUST publish that image tagged with the 7-character
`git rev-parse --short` commit hash and MUST advance `latest` to the same
image index. A `v*` git tag MUST publish the same image with that tag and MUST
also advance `latest`. Published tags MUST provide `linux/amd64` and `linux/arm64`
manifests. The image web stage MUST install this repository with the frozen
lockfile and build the Vite application in `web/`.
The Rust and Web build stages MUST execute on the native build platform; the
Rust stage MUST cross-compile the requested target architecture rather than
compile the ARM64 dependency graph through QEMU.
The default runtime image MUST omit Homebrew and bundled agent distributions,
retain checksum-verified upstream jj 0.45.1 on both architectures, and load
optional agent runtimes through OCI Mods. jj artifact pins MUST reuse the CI
installer's source of truth. Normal tags and compatibility `-slim` tags MUST
reference the same image index, retain the same server, static UI, unprivileged
application, readiness and `/data` contract, and stay below 500 MiB unpacked.
Both Docker build targets MUST copy application files read-only without
later permission layers duplicating their contents. Nonempty legacy Homebrew
package requests MUST fail with a migration diagnostic rather than be ignored.
No image build or startup path may install Homebrew. Compose MUST stop mounting
Homebrew storage without deleting existing volumes or modifying profile data.
Mods follow the [reusable container extension contract](./mods.md).
Hook failures retain bounded launch-error or child-exit diagnostics under
SRV-MOD-006; these diagnostics do not retry initialization hooks.
`lapis-ai-service` stays the git dependency
`https://github.com/lapismd/lapis-ai` tag `v0.2.0` at version `0.2.0`.

The repository root Compose file MUST build the runtime image from this
checkout and bind the demo port to loopback. `scripts/demo.sh` MUST default to
the `default` Docker context, prepare `.env` from `.env.example`, an empty-vault
config and a random mounted bootstrap password without overwriting existing
files, reject a Docker memory allocation below 5 GiB with a 6 GiB build
allocation diagnostic, build Web and Rust stages serially, then wait for
container readiness. Local demo configuration and secrets
MUST be ignored by version control and excluded from the image build context.
The demo script's `image IMAGE` mode MUST pull the explicit published image and
start Compose with `--no-build`, retaining local configuration and data volumes
without requiring source-build memory.

### Acceptance details

- The Dockerfile exposes port 8787, volume `/data`, and the readyz URL, while Cargo.toml pins lapis-ai-service to tag v0.2.0 and version 0.2.0.
- Main publishes the 7-character commit, `latest` and compatibility `-slim` tags from one multiarchitecture image index; v\* tags publish their version, `latest` and both compatibility aliases.
- The image cross-compiles each target Rust executable and builds `web/` with Vite on the native build platform after a frozen lockfile install.
- Both architectures run jj version, repository initialization and status as UID 1993; OCI executable/hooks, read-only no-Mod boot, graceful shutdown, unsupported Homebrew request rejection and the 500 MiB limit remain required.

## SRV-SRV-002 — jj-store git revision

<a id="srv-srv-002"></a>

**Requirement.** History, vault, and repository crates MUST use the Cargo git
dependency `https://github.com/stevejuma/jj-store` revision
`086def182eae2ab6e5dc39aba030390d7ea8612c`. This repository MUST NOT use a
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

The native server crate also exports an offline OpenAPI generator for the
registered HTTP routes. The executable and Swagger UI use the same catalog.
The server serves the shared PWA raster asset at the public icon URL advertised by initialized MCP under
[SRV-SRV-018](integration-auth.md#srv-srv-018--oauth-for-remote-mcp-clients).

### Acceptance details

- The workspace contains those six crates; HTTP/MCP adapters share Core/Protocol without server bootstrap or Tauri, while the CLI depends on Protocol and uses native owners only in tests.
- The core and server manifests have no path dependency that leaves this repository.
- The Notes hosted-server chapter points at this book instead of restating it.
- Default `local-sibling-projections` builds and tests the retained engines through Git dependencies, never external workspace paths.

## Native embedding cache lease

Native embedding-cache installers retain one exclusive lease through verified
publication. Releasing the owner explicitly unlocks the lease before closing
its descriptor, including when a concurrent process has inherited a duplicate.
This preserves the existing wait deadline and immediate busy refusal.

## Native engine source intake

Native Markdown/Search algorithms use lapis-plugins commit
`9f2713ac6a4f40ff48325620d969de08fb31f5aa`, Tasks uses
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
The demonstrated command-lease repair and its unchanged admission boundary are
recorded in the [lease release receipt](../../records/2026-11-10-vault-backup-lease-release.md).

## Shared content automation

The [content automation contract](content.md) owns SRV-SRV-005 through
SRV-SRV-014, including shared host controls and hosted plugin settings. Hosts still own authentication,
consent, persistence, admission and listener qualification under SRV-SRV-004.
CLI connection instructions locate hosted content settings under Core plugins;
this documentation correction changes no CLI or native service behavior.

<a id="srv-srv-005"></a>

[SRV-SRV-005](content.md#srv-srv-005) moved to the content chapter.

<a id="srv-srv-006"></a>

[SRV-SRV-006](content.md#srv-srv-006) moved to the content chapter.

<a id="srv-srv-007"></a>

[SRV-SRV-007](content.md#srv-srv-007) moved to the content chapter.

<a id="srv-srv-008"></a>

[SRV-SRV-008](content.md#srv-srv-008) moved to the content chapter.

<a id="srv-srv-009"></a>

[SRV-SRV-009](content.md#srv-srv-009) moved to the content chapter.

<a id="srv-srv-010"></a>

[SRV-SRV-010](content.md#srv-srv-010) moved to the content chapter.

<a id="srv-srv-011"></a>

[SRV-SRV-011](content.md#srv-srv-011) moved to the content chapter.
The hosted adapter mounts [MCP Apps](mcp-apps.md) from private installation
assets when configured.

<a id="srv-srv-012"></a>

[SRV-SRV-012](content.md#srv-srv-012) moved to the content chapter.

Core owns client-scoped Terminal Host PTY attachments. Client retirement and successful vault close drain shells outside the vault state lock; dirty-editor refusal leaves sessions live. Portable Terminal commands and bounded output types are generated with the other protocol contracts. The private engine is pinned to its reviewed Terminal Host Git revision.

Terminal HTTP observations release their connection after a bounded response
lease, even when the shell remains alive. Byte cursors preserve replay across
reconnections; the browser yields connection capacity to editor commands.

[SRV-HST-021](hosted.md#srv-hst-021) owns native file-change observation, replacing recursive browser filesystem polling without changing file-byte routes, admission or durable writers.

The source adapter is private host state, separate from repository connections.
Native profile startup selects installation-private remote storage. Authenticated
remote inventory/controls and working-copy routes require the attached vault and
generation in addition to existing protocol/owner/origin/CSRF admission. Accepted
work remains Core-owned after response cancellation; retirement fences delivery.
The single installation owner has a stable private identity independent of login
sessions. Token-only and browser authentication refer to that same owner.

Operator configuration selects `LAPIS_SERVER_REPOSITORY_KEY_FILE` outside the
profile and every vault, optional GitHub client ID and SSH known-hosts path.
Missing/wrong keys expose locked custody; SSH remains independently available.
The runtime includes `openssh-client` and preserves host-key verification.
[Operator instructions](../../../docs/repository-remotes.md) define provisioning,
persistence and key recovery. Browser adoption and cold transport acceptance
remain in the native-services backlog until independently qualified.

## SRV-SRV-015 — Encrypted repository credential custody

<a id="srv-srv-015"></a>

**Requirement.** Server repository tokens and forge account credentials MUST use authenticated encryption at rest with a separately operator-mounted 32-byte key. Missing or incorrect keys and corrupted stores MUST remain locked without plaintext fallback. Authenticated records MUST bind installation, authenticated owner and credential identity.

### Acceptance details

- Verify locked states for absent, malformed, incorrect and installation-mismatched keys, without rewriting encrypted records.
- Prove authenticated owner/record identity binding, tamper rejection and encrypted restart persistence using disposable private storage.

## SRV-SRV-016 — Scoped repository credential lifecycle

<a id="srv-srv-016"></a>

**Requirement.** Native credential custody MUST support durable save, replacement and forgetting through opaque references. Secret-free inventory MUST omit stored tokens, and resolution MUST enforce the approved HTTPS endpoint/account scope and owner. Browser requests MUST NOT select key files or storage locations; vault checkpoints and exports MUST exclude credential storage.

### Acceptance details

- Save, replace and forget retain opaque references across restart; native inventory never includes stored tokens.
- Wrong-owner and wrong-endpoint resolution fail, while HTTP adapters retain authenticated admission and never accept custody paths.

## Shared backup remote selection

The admitted browser transport uses only its existing client descriptor for
remote-settings and working-copy requests, including the exact vault/generation
query. It fences delivery after detach and never replays a remote mutation after
a lost response. Working-copy transport preserves raw native JSON. This port
does not itself enable shared settings or credential/browser handoff.

The separate browser remotes adapter forwards native command/snapshot shapes
and retains its admitted lifetime. Sign-in handoff reads the current pending
authorization, copies its code before opening the HTTPS device page and refuses
delivery after detach or repository-observer retirement. It adds no renderer
credential persistence or native-work cancellation. Production composition and
real OAuth acceptance remain separate from private adapter qualification.

Core owns the device-private backup remote selection policy extracted from
Desktop. Existing version-one records, configuration revisions, fetched anchors
and native-derived pending-publication identities remain compatible. Saving
selection performs no transfer. Active publication prevents rebinding or
disconnection; observation cannot overwrite a newer configuration revision.
Host adapters retain account resolution, authentication, paths and admission.

Remote management commands generated by Protocol MUST select one repository
target and expected native configuration revision. Push execution MUST reference
one native review, without force/all-bookmark or arbitrary destination fields.
Native host paths, authenticated owner and credential-store locations MUST NOT
be request fields. Credential input has no printable or serializable Rust
implementation; response inventory contains opaque metadata only.

CLI registration is owned by [SRV-CLI-001](cli.md#srv-cli-001). Runtime bundling qualification is tracked separately from standalone registration and release archives.

The runtime image includes `lapis` on the unprivileged runtime user's PATH.
Hosted General → Advanced registers browser-computer download guidance through
UI's App-independent CLI settings entry point. The host consumes its published
`@lapis-notes/ui@^0.2.0` registry export. Release/archive and image qualification
remain distinct gates.

Bundled CLI removal keeps its setup lease until shell cleanup finishes, including
PATH-only cleanup after command removal. Server and Desktop consume this rule.

Detailed Core selection, synchronization, credential and publication policy
lives in [Repository remote operations](repository-remotes.md).

CLI executable-fixture qualification preserves SRV-CLI-001 production policy.
The alternate-source fixture is copied by a completed child process so parallel
process starts cannot inherit its executable writer descriptor. All existing
source-switch, repair and tamper-refusal assertions remain unchanged.

The hosted native History composition MUST supply the admitted native editing-
intent bridge even when Tasks is unselected. Opted-in native backup merging
cannot rely on a Tasks feature flag to learn about unsaved editor buffers.

Packaged skill source and installation contracts are defined in [SRV-OPS-005–007](plugin-operations.md). Native services own their writes; host adapters supply immutable packaged resources.

The native CLI skill contribution embeds its linked command, credential and
skill-management references. Nested resources are owned by the same versioned
receipt and complete-directory update guard as the entrypoint.

The hosted skill-management integration consumes public App Profile `^0.2.1`,
API `^0.6.5` and their compatible registry cohort. Source/archive acceptance
and final frozen-registry acceptance remain separate release gates.

Current-main skill intake preserves remote-profile startup and public registry
packages; [qualification evidence](../../records/2026-11-15-server-skill-profile-intake.md)
separates earlier source proofs from the combined release candidate.

The bundled Markdown skill intake requires `@lapis-notes/markdown@^0.3.2`
or newer compatible releases; 0.3.1 has no supporting reference resources.

Hosted qualification builds the CLI alongside the server before app/CLI journeys.

Native terminal launch arguments/environment are supplied by the owning Server Host through Core's optional per-vault factory. The default Core/PTY constructors remain unchanged; [native profile qualification](../../records/2026-11-20-native-terminal-profile.md) covers startup hooks, status, real paths, admission and opt-out. Prompt checks distinguish visible ANSI text from non-rendering OSC metadata, whose cwd retains its full physical path.

The native operation registry supplies installed descriptors to the packaged
MCP Apps loader. Transport discovery still checks each operation’s current
admission and granted permissions.

The CLI's [approved Desktop connection reuse](cli.md#approved-desktop-connection-reuse)
resolves host-selected connection metadata before ordinary content, operation and
MCP dispatch. The CLI never admits a vault or constructs a second native owner.
Remote URL targets preserve their explicit credentials and transport boundaries. CLI version custody follows [the CLI contract](cli.md#cli-release-identity).

Android conditional replacement stages a named file in the admitted parent directory. The writer preserves permission and modification-time checks, checks the expected bytes again before rename, and synchronizes the parent before acknowledging the edit.

Mobile hosts can opt into the typed database with the mobile-database feature. The native owner admits an absolute app-private database directory and vault UUID. Renderer calls enter through the client authority check. Lexical search and token-hash embeddings run locally. Mobile builds reject unregistered model runtimes at configuration time.
