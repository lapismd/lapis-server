# Hosted server

This book owns the hosted server. Shared Notes product requirements stay in
the Notes specification at <https://lapismd.github.io/lapis-notes/>.
The core crate README routes native ownership and commands here; outstanding
integration and qualification live in the [canonical backlog](../backlog.md).

The [plugin operation contract](plugin-operations.md) extends shared Core and
Protocol with namespaced descriptors and credential-bound dispatch. It does not
alter native engine ownership or the crate dependency direction in SRV-SRV-004.
The compiled Tasks adapter incorporates its owning repository's current-main
intake at a pinned Git revision. Workspace patches bind Core/Protocol to this host's
internal workspace instances without introducing an external Cargo path.

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
- Main publishes the 7-character commit and compatibility -slim tags from one multiarchitecture image index; v\* tags publish both aliases.
- The image cross-compiles each target Rust executable and builds `web/` with Vite on the native build platform after a frozen lockfile install.
- Both architectures run jj version, repository initialization and status as UID 1993; OCI executable/hooks, read-only no-Mod boot, graceful shutdown, unsupported Homebrew request rejection and the 500 MiB limit remain required.

## SRV-SRV-002 — jj-store git revision

<a id="srv-srv-002"></a>

**Requirement.** History, vault, and repository crates MUST use the Cargo git
dependency `https://github.com/stevejuma/jj-store` revision
`6649b7e95ee58823ba28f0a9ca01e507b458edd9`. This repository MUST NOT use a
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

<a id="srv-srv-012"></a>

[SRV-SRV-012](content.md#srv-srv-012) moved to the content chapter.

Core owns client-scoped Terminal Host PTY attachments. Client retirement and successful vault close drain shells outside the vault state lock; dirty-editor refusal leaves sessions live. Portable Terminal commands and bounded output types are generated with the other protocol contracts. The private engine is pinned to its reviewed Terminal Host Git revision.

Terminal HTTP observations release their connection after a bounded response
lease, even when the shell remains alive. Byte cursors preserve replay across
reconnections; the browser yields connection capacity to editor commands.

[SRV-HST-021](hosted.md#srv-hst-021) owns native file-change observation, replacing recursive browser filesystem polling without changing file-byte routes, admission or durable writers.

The source adapter is private host state, separate from repository connections.
Host route admission, shared remote-service integration and cold acceptance remain
in the native-services backlog until independently qualified.

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

Core owns target-aware private remote selection independently of secret custody
and repository configuration. Saving a new or changed backup connection MUST
leave automatic synchronization off. Original selection never changes that
backup preference. Exact configuration revisions and pending-publication guards
fence stale writes; records contain validated endpoints and opaque credential
references, never token bytes or host filesystem paths.

Forge credential ciphertext uses a separate authenticated domain and the full
256-bit account identity, alongside installation and owner. Account metadata
and token inventory remain independent. Replacing/forgetting forge secrets
preserves the stable account identity; no stored secret crosses into browser
inventory. Missing keys and authentication failures never create plaintext
fallback records. Host forge-manager and route adoption remain separate gates.

Shared remote commands MUST acquire native accepted-work ownership before
waiting for their serialized turn. Discarding a response or detaching its client
cannot abandon an accepted private configuration write; delivery is fenced after
completion. A later admitted client MUST use the same vault state without being
bound to the retired client. No per-vault remote owner may retain a permanent
work lease that prevents vault closure. Settings projection grants no transport
or original mutation authority.

Working-copy status MUST read an admitted existing Git-backed JJ original
without snapshotting files, changing checkout metadata or moving history pins.
Its frame identifies that source and includes complete bounded file/line totals;
unsupported or unstable observations remain unavailable rather than invented
zeros. With no original, an already initialized managed backup MAY report its
latest recorded checkpoint and native parent-diff totals. This status MUST omit
later unrecorded disk bytes, use the existing native diff engine, and retain
unknown line counts for binary/unsupported content. It MUST NOT initialize a
repository, start capture, move a pin or silently fall back after an invalid
original-source witness. Native source witnesses remain independent of mutation capabilities and client-owned
history handles.

Passive original remote discovery MUST use the immutable native constructor,
without obtaining a mutation capability from a history/source reader. It lists
existing remote names and sanitized optional URLs and eligible local bookmarks;
credential-bearing or unsupported URLs never cross the host boundary. Discovery
neither selects `origin` nor writes Git configuration, initializes transport,
enables synchronization or advertises fetch/push authority. Existing clients
may omit the additive discovered-remotes field.

Explicit original connection saves MUST bind private selection to the witnessed
physical source and exact discovered Git configuration revision. External
configuration changes advance the private observation epoch and refuse stale
settings writes. The native Git configuration transaction runs under the private
selection process lock, after credential scope and pending-publication checks.
If Git config succeeds but private persistence fails, the previous selection
cannot authorize a mismatched endpoint or source. Unrelated remotes survive;
saving does not initialize transport, transfer data or change working-copy state.
Anonymous HTTPS and host SSH remain valid with locked token storage; missing,
wrong-host or unavailable token/account references refuse before Git writes.
Only explicit accepted saves or separately admitted manual transfer commands
construct the separate native writer capability; passive source witnesses and
History handles never acquire it.

Native host composition MUST explicitly admit transfers independently of
credential inventory. Test, fetch, push review, execution, observation and stop
commands MUST hold the private cross-process selection guard for their accepted
operation. Stale revisions, source/configuration changes and invalid review or
request identities refuse before credential resolution or network access.
Test connection observes remote reachability/authentication without fetching or
publishing. Anonymous reads and host SSH need no unlocked token store.

Original fetch MUST use native JJ bookmark/tracking semantics without capturing
files, writing checkout state or retargeting a client-owned History pin. Status
after a metadata-only fetch MUST remain readable through the qualified native
immutable observer; application code MUST NOT repair the checkout pointer or
substitute invented counts. Observations retain real bounded incoming/outgoing
counts and their fetch time independently for the selected connection.

Original push reviews MUST persist outside the vault, bound to the physical
source, exact saved connection and native immutable review. The public response
contains only an opaque review identity, selected commit and safe summary/counts.
Execution consumes that identity durably before invoking native publication,
which revalidates its operation, endpoint, bookmark and ancestry and reserves a
stable receipt before network publication. Consumed reviews cannot replay.
Pending publication MUST survive detach/restart, prevent new publication or
rebinding, and expose its original identity for observe-only recovery or an
explicit local stop. Stopping requires no credentials and cannot claim to undo a
possibly accepted remote update. Native custody resolves scoped secrets only
inside the accepted backend operation; stored secrets never become DTO fields.

Managed backup test/fetch/review/publication/recovery MUST use the same explicit
target and private selection admission, independently of original repositories.
Fetched metadata MUST enter through the running native capture worker when one
exists. With no running worker, explicit metadata import MUST NOT start capture
or snapshot files. Network transfer and immutable review/validation MUST remain
independent of the capture lease; application code MUST NOT mask native races
with Busy retries.

Backup review MUST pin an existing recorded checkpoint and native intent/counts.
A later local capture cannot replace that reviewed commit or add newly saved
files to its publication. Missing local history remains unknown and cannot be
reported as zero outgoing changes. Source/connection-bound private journals and
their bounded storage remain independent for the two targets. Native pending
receipts survive restart, prevent rebinding/new publication, and permit
observe-only recovery or an explicit credential-free local stop. New backup
connections and later local captures MUST remain unpublished until an explicit
push or separately persisted synchronization opt-in.

Credential mutations MUST execute through backend-only custody with the same
accepted-work lifetime and an installation-scoped process fence. An observation
epoch MUST be durably reserved before provider effects; every vault MUST observe
that epoch before admitting another command, fencing stale credential settings
across targets and vaults even after a lost mutation response. Remote selection
records contain only the epoch and opaque references, never submitted secrets.
Replacement MUST preserve endpoint/account scope. An unresolved publication MUST
retain its referenced credentials; terminal native outcomes retire its review
hold without permitting replay. Sign-in completion that could replace a retained
account MUST wait for observation or explicit local stop.
Custody futures MAY be single-threaded inside the accepted native executor;
they MUST NOT cross the worker/task boundary or retain a renderer lifetime.

Native credential composition shares account inventory, endpoint/account scope,
identity-preserving token replacement and SDK device authorization in Core.
Hosts inject their admitted-owner raw-token store and forge manager with private
metadata and separate secret custody. No renderer request chooses a store root.
A locked provider retains host SSH capability but disables token/account access;
private device authorization sessions never cross the Protocol boundary. Saved
forge identity comes from the verified provider, not submitted display fields.
