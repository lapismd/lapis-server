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

Use `./scripts/demo.sh image ghcr.io/lapismd/lapis-server:<tag>` to pull a published image without building. The root Compose file also supports checkout builds. It publishes port
`8787`, keeps application files read-only, and stores the profile in the
`lapis_data` volume mounted at `/data`. The default image includes `jj` 0.45.1
and does not include Homebrew or agent runtimes.

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

## JJ and agent Mods

The default image supplies checksum-verified upstream `jj` on both published
architectures. Normal and `-slim` tags refer to the same minimal image.
Use the [Mod configuration guide](../README.md#configure-agent-mods) to select
Codex, Cursor, Claude Code or OpenCode. Provider authentication and
`LAPIS_SERVER_AGENTS_CONFIG` remain operator inputs.

Mods require root initialization (`user: "0:0"`), a writable container layer
and a separate cache volume. The loader starts the original application as
UID/GID 1993. Keep the existing profile volume and Compose project identity
when upgrading. Old Homebrew volumes are preserved but no longer mounted;
remove obsolete `LAPIS_SERVER_HOMEBREW_*` package settings.
