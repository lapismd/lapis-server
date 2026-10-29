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

## Credentials

Compose reads the bootstrap password from `LAPIS_SERVER_PASSWORD_FILE`. Point
that variable at a file outside this checkout. The compose secret mounts the
file at `/run/secrets/lapis_bootstrap_password`. Do not commit the password
file, and do not pass the password on the command line. That file is the
owner password only when the profile has no owner yet. An existing profile
keeps the password from its first start. The bootstrap username is
`LAPIS_SERVER_BOOTSTRAP_USERNAME`.

## Build context

`crates/lapis-server/Dockerfile` builds from this `lapismd/lapis-server`
checkout. The repository root is the Docker context. jj-store crates come from
git revision `7a98bd89d2bc77fb6ec41f91186992abb19ba161`. `lapis-ai-service` is
fetched from lapis-ai tag `v0.2.0`. Publishing the image is a separate
authorization and is not part of this pull request.
