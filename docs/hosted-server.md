# Hosted Lapis server image

The public server image is `ghcr.io/lapismd/lapis-server`. It is the compiled
Rust host and the existing Web application. It is not
`ghcr.io/lapismd/lapis-notes-ci` or `ghcr.io/lapismd/lapis-ci`.

This page describes how to run a published image. Publishing the image is a
separate step and is not part of preparing the package release pull request.

## Run

From a `lapismd/lapis-server` checkout:

```sh
export LAPIS_SERVER_CONFIG_FILE="$PWD/config.json"
export LAPIS_SERVER_PASSWORD_FILE="$PWD/password.txt"
docker compose up -d
```

`docker-compose.yml` pulls `ghcr.io/lapismd/lapis-server`. It publishes port
`8787`, keeps application files read-only, and stores the profile in the
`lapis_data` volume mounted at `/data`.

Set `LAPIS_SERVER_ORIGIN` to the exact browser origin. Leave
`LAPIS_SERVER_INSECURE_COOKIE=1` only for local HTTP. Behind HTTPS, set the
origin to that public URL and set `LAPIS_SERVER_INSECURE_COOKIE=0`.

## Readiness

The process listens on `8787`. Liveness is `http://127.0.0.1:8787/healthz`.
Readiness is `http://127.0.0.1:8787/readyz`. A ready host returns `ready`.
During drain the same path returns `unavailable` with HTTP 503.

```sh
curl --fail --silent http://127.0.0.1:8787/readyz
```

## Build context

`crates/lapis-server/Dockerfile` builds from a `lapismd/lapis-notes` checkout.
The repository root is the Docker context and is copied to `/work/lapis-notes`.
`pnpm build:server:container` still qualifies the local image
`lapis-server:rust-local` by staging path-pinned sibling crates into that
checkout before the build. Those path dependencies stay in jj-store,
lapis-plugin-docs, lapis-plugin-tasks, and lapis-plugins because those
repositories have no crate tags. `lapis-ai-service` is fetched from lapis-ai
tag `v0.2.0` during the image build.
