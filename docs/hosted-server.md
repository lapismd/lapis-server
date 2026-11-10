# Hosted Lapis server image

The public server image is `ghcr.io/lapismd/lapis-server`. Published tags
provide `linux/amd64` and `linux/arm64` manifests. It is the compiled Rust host
and the existing Web application. It is not
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
`lapis_data` volume mounted at `/data`. Homebrew and its installed packages use
the independent `lapis_homebrew` volume mounted at
`/home/linuxbrew/.linuxbrew`.

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

## Homebrew and agent CLIs

The image includes `brew`, `codex`, `cursor-agent`, `claude`, and `opencode` on
both published architectures. Run Cursor as an ACP server with
`cursor-agent acp`. Provider authentication and
`LAPIS_SERVER_AGENTS_CONFIG` remain operator inputs; the image does not create
agent targets or embed credentials.

The named Homebrew volume is seeded from a new image and subsequently keeps
its own installed versions. Do not replace it with an empty bind mount. Set
`LAPIS_SERVER_HOMEBREW_PACKAGES` to a whitespace- or newline-separated list of
additional formulae and `LAPIS_SERVER_HOMEBREW_CASKS` to the equivalent cask
list. Startup validates every name and fails on installation errors. Formulae
must have compatible bottles because the runtime intentionally omits build
tools.

Homebrew's implicit updates are disabled. Operators can update the retained
state explicitly:

```sh
docker compose exec lapis-server brew update
docker compose exec lapis-server brew upgrade
```

## Build context

`crates/lapis-server/Dockerfile` builds from this `lapismd/lapis-server`
checkout. The repository root is the Docker context. jj-store crates come from
git revision `076e418cecbe204925aea874bef8aa8a680269a9`. `lapis-ai-service` is
fetched from lapis-ai tag `v0.2.0`. Publishing the image is a separate
authorization and is not part of this pull request.
