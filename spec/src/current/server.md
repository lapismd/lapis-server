# Hosted server

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

## SRV-SRV-002 — jj-store path until a published revision

<a id="srv-srv-002"></a>

**Requirement.** History, vault, and repository crates MUST stay path
dependencies on a local jj-store checkout until
https://github.com/stevejuma/jj-store/pull/22 merges and a commit or tag exists
on https://github.com/stevejuma/jj-store. This repository MUST NOT point those
crates at pull request 22 head `35356e655765143b29a6c9b1518ef3b91d04837d` and
MUST NOT invent a tag. After that pull request merges, the paths are replaced
by a Cargo git dependency on https://github.com/stevejuma/jj-store at the
merged revision. Published main `c859bb30d2a2ae7bd38ce5273a76557598a7f2de` is
not that replacement. There is no jj-store crate tag.

### Acceptance details

- The core manifest uses a jj-store path dependency.
- The configure script rejects a jj-store git URL and the unmerged head.
