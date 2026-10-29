# Lapis server

[![Server CI](https://github.com/lapismd/lapis-server/actions/workflows/ci.yml/badge.svg)](https://github.com/lapismd/lapis-server/actions/workflows/ci.yml)
[![Storybook Pages](https://github.com/lapismd/lapis-server/actions/workflows/publish-spec-pages.yml/badge.svg)](https://github.com/lapismd/lapis-server/actions/workflows/publish-spec-pages.yml)
[![Storybook](https://img.shields.io/badge/storybook-live-ff4785?logo=storybook&logoColor=white)](https://lapismd.github.io/lapis-server/storybook/)

Lapis server is the hosted application for Lapis Notes. One process serves the
Rust host and the Notes workspace. The workspace UI comes from the published
`@lapis-notes/workspace` package. Shared Notes packages stay in lapis-notes and
install from the npm registry.

Specification and Storybook:
<https://lapismd.github.io/lapis-server/storybook/>.

## Run

The image `ghcr.io/lapismd/lapis-server` is published when a `v*` git tag is
pushed. That tag is applied to the image. No image tag has been published yet.
After one exists, replace `<v*>` and run it:

```sh
docker pull ghcr.io/lapismd/lapis-server:<v*>
docker run --rm \
  --publish 8787:8787 \
  --volume lapis-data:/data \
  --volume "$LAPIS_SERVER_CONFIG_FILE":/run/lapis/config.json:ro \
  --volume "$LAPIS_SERVER_PASSWORD_FILE":/run/secrets/lapis_bootstrap_password:ro \
  --env LAPIS_SERVER_ORIGIN=http://127.0.0.1:8787 \
  --env LAPIS_SERVER_INSECURE_COOKIE=1 \
  --env LAPIS_SERVER_BOOTSTRAP_USERNAME=owner \
  --env LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE=/run/secrets/lapis_bootstrap_password \
  ghcr.io/lapismd/lapis-server:<v*>
```

The container listens on port `8787`, stores the profile on the `/data` volume,
and reports readiness at <http://127.0.0.1:8787/readyz>.

```sh
curl --fail --silent http://127.0.0.1:8787/readyz
```

`LAPIS_SERVER_CONFIG_FILE` is a JSON config on the host, mounted read-only at
`/run/lapis/config.json`. `{"vaults":[]}` is enough for the first start. For
local HTTP, set `LAPIS_SERVER_ORIGIN` to `http://127.0.0.1:8787` and
`LAPIS_SERVER_INSECURE_COOKIE` to `1`.

## Credentials

`LAPIS_SERVER_BOOTSTRAP_PASSWORD`, when set to a non-empty value, is the
initial owner password and overrides the password file. Leave it unset or empty
to use the file. Pass the variable through the environment
(`--env LAPIS_SERVER_BOOTSTRAP_PASSWORD`) so the value stays out of the command
line. The server does not log it.

The password file stays at the path Compose already uses. Set
`LAPIS_SERVER_PASSWORD_FILE` to a file outside this checkout. The container
mounts that file at `/run/secrets/lapis_bootstrap_password`, and the server
reads it as `LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE`. Keep the file out of the
repository.

That password is used only when the profile has no owner yet. An existing
profile keeps the password from its first start. The bootstrap username is
`LAPIS_SERVER_BOOTSTRAP_USERNAME`.

## Develop

```sh
pnpm install
pnpm storybook
```

`pnpm spec:check` and `pnpm format:check` check the specification.
`pnpm configure` checks the package manifests.
