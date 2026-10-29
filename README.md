# Lapis server

[![Server CI](https://github.com/lapismd/lapis-server/actions/workflows/ci.yml/badge.svg)](https://github.com/lapismd/lapis-server/actions/workflows/ci.yml)
[![Specification Pages](https://github.com/lapismd/lapis-server/actions/workflows/publish-spec-pages.yml/badge.svg)](https://github.com/lapismd/lapis-server/actions/workflows/publish-spec-pages.yml)
[![Storybook](https://img.shields.io/badge/storybook-live-ff4785?logo=storybook&logoColor=white)](https://lapismd.github.io/lapis-server/storybook/)

This repository is the UI for operating Notes, backed by this server. It mounts
`WorkspaceShell` from the published `@lapis-notes/workspace` package. Shared
Notes packages stay in lapis-notes and resolve from the npm registry.

Specification: <https://lapismd.github.io/lapis-server/>.
Storybook: <https://lapismd.github.io/lapis-server/storybook/>.

## Run

```sh
pnpm install
pnpm spec:check
pnpm configure
pnpm storybook
```

`pnpm configure -- --metadata` asks Cargo to read the package graph. That
resolves the pinned jj-store revision. It is not a compile and not a release.

The public image is `ghcr.io/lapismd/lapis-server`. It listens on port `8787`,
stores the profile on `/data`, and reports readiness at
`http://127.0.0.1:8787/readyz`. Publishing that image is a separate
authorization. Local image commands are in
[Hosted server image](docs/hosted-server.md).

## Credentials

Compose reads the bootstrap password from `LAPIS_SERVER_PASSWORD_FILE`. Point
that variable at a file outside this checkout. The compose secret mounts the
file at `/run/secrets/lapis_bootstrap_password`. Do not commit the password
file, and do not pass the password on the command line. That file is the
owner password only when the profile has no owner yet. An existing profile
keeps the password from its first start. The bootstrap username is
`LAPIS_SERVER_BOOTSTRAP_USERNAME`.

The Rust workspace is `lapis-server`, `lapis-app-core`, and
`lapis-app-protocol`. History and vault crates use Cargo git revision
`7a98bd89d2bc77fb6ec41f91186992abb19ba161` from
`https://github.com/stevejuma/jj-store`. There is no crate tag. Do not use
pull request 22 head `35356e655765143b29a6c9b1518ef3b91d04837d`.

`lapis-ai-service` is git tag `v0.2.0` at version `0.2.0`.
