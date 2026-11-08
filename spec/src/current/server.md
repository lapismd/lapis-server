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
- Main publishes the 7-character commit hash and both supported architecture manifests, and a v\* tag publishes that version tag with both manifests.
- The image web stage builds `web/` with Vite after a frozen lockfile install.
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

**Requirement.** This repository MUST own the `lapis-server`, `lapis-app-core`, and `lapis-app-protocol` crates, and Cargo paths MUST stay internal. Shared Notes product requirements MUST stay in Notes. Retained native projection engines MUST compile by default through `local-sibling-projections` using exact Git commits. Compilation MUST NOT change per-vault native ownership or configuration.

### Acceptance details

- The Cargo workspace contains those three crates.
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
