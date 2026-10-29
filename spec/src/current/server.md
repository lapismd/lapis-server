# Hosted server

This book owns the hosted server. Shared Notes product requirements stay in
the Notes specification at <https://lapismd.github.io/lapis-notes/>.

## SRV-SRV-001 — Public image

<a id="srv-srv-001"></a>

**Requirement.** The server image MUST be `ghcr.io/lapismd/lapis-server`. It
MUST listen on port `8787`, store the profile on volume `/data`, and report
readiness at `http://127.0.0.1:8787/readyz`. It MUST NOT be
`ghcr.io/lapismd/lapis-notes-ci` or `ghcr.io/lapismd/lapis-ci`.
`lapis-ai-service` stays the git dependency
`https://github.com/lapismd/lapis-ai` tag `v0.2.0` at version `0.2.0`.

### Acceptance details

- The Dockerfile exposes port 8787, volume `/data`, and the readyz URL.
- Cargo.toml pins lapis-ai-service to tag v0.2.0 and version 0.2.0.

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

**Requirement.** This repository MUST own the `lapis-server`, `lapis-app-core`, and `lapis-app-protocol` crates. Cargo path dependencies MUST stay inside this repository. Shared Notes product requirements MUST stay in the Notes specification. Projection and task fields that only sibling projection code reads MUST be compiled only with `local-sibling-projections`.

### Acceptance details

- The Cargo workspace contains those three crates.
- The core and server manifests have no path dependency that leaves this repository.
- The Notes hosted-server chapter points at this book instead of restating it.
- The default workspace build compiles sibling-projection fields only with `local-sibling-projections`.
