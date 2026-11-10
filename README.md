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

## Local demo from this checkout

From the repository root, run:

```sh
./scripts/demo.sh
```

This uses the local `default` Docker context, copies `.env.example` to `.env`,
and creates an empty-vault config and a random initial owner password under
`.local/demo`. Existing files are preserved. Docker with Compose and OpenSSL
are required, with at least 6 GiB RAM allocated to Docker for the source
build. The script checks available Docker memory before building. Node and Rust
are built inside Docker. The first build also
installs the bundled agent tools and can take several minutes. The script
builds Web and Rust stages serially to reduce peak memory use in local Docker.

Open <http://localhost:8787>, sign in as `owner`, and read the password with
`cat .local/demo/bootstrap-password`. Create a vault from the vault chooser.
The port is bound to loopback. Both `.env` and `.local` are ignored by version
control and excluded from the Docker build context.

To edit settings before the first build, run `./scripts/demo.sh prepare` and
edit `.env`. If you change the port, update `LAPIS_SERVER_PORT` and
`LAPIS_SERVER_ORIGIN` together. Custom config/password paths must point to
existing files; the script only creates the default `.local/demo` files.
The initial password only applies to a new data volume.

Once configured, the root Compose file can also be used directly:

```sh
docker --context default compose up --build --detach --wait
docker --context default compose logs --follow
docker --context default compose down
```

Stopping preserves the profile and Homebrew named volumes. On Docker Desktop,
enable “Allow the default Docker socket to be used” if `/var/run/docker.sock`
is missing. To explicitly choose another local context, set
`LAPIS_DEMO_DOCKER_CONTEXT=desktop-linux` when running the script.

## Run a published image

The same local configuration and volumes can run a published image without
compiling the checkout:

```sh
LAPIS_DEMO_DOCKER_CONTEXT=desktop-linux ./scripts/demo.sh image ghcr.io/lapismd/lapis-server:45f4c1b
```

Omit the context override when the local `default` context is available.
Image mode pulls the requested tag, skips the build-memory preflight, and
starts Compose with `--no-build`. For direct Compose use, set
`LAPIS_SERVER_IMAGE` to the desired published tag and pass `--no-build`.

Every push to `main` publishes `ghcr.io/lapismd/lapis-server` tagged with the
7-character commit hash from `git rev-parse --short`. A `v*` git tag publishes
the same image with that tag and `latest`. Published tags include `linux/amd64`
and `linux/arm64` manifests. Replace `<short>` or `<v*>` and run it:

```sh
docker pull ghcr.io/lapismd/lapis-server:<short>
docker pull ghcr.io/lapismd/lapis-server:<v*>
docker run --rm \
  --publish 8787:8787 \
  --volume lapis-data:/data \
  --volume lapis-homebrew:/home/linuxbrew/.linuxbrew \
  --volume "$LAPIS_SERVER_CONFIG_FILE":/run/lapis/config.json:ro \
  --volume "$LAPIS_SERVER_PASSWORD_FILE":/run/secrets/lapis_bootstrap_password:ro \
  --env LAPIS_SERVER_ORIGIN=http://127.0.0.1:8787 \
  --env LAPIS_SERVER_INSECURE_COOKIE=1 \
  --env LAPIS_SERVER_BOOTSTRAP_USERNAME=owner \
  --env LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE=/run/secrets/lapis_bootstrap_password \
  ghcr.io/lapismd/lapis-server:<v*>
```

The container listens on port `8787`, stores the profile on the `/data` volume,
keeps Homebrew state in `/home/linuxbrew/.linuxbrew`, and reports readiness at
<http://127.0.0.1:8787/readyz>.

```sh
curl --fail --silent http://127.0.0.1:8787/readyz
```

## Smaller server-only image

Append `-slim` to the same release or commit tag, for example
`ghcr.io/lapismd/lapis-server:<short>-slim`. It serves the same UI and native
server on AMD64 and ARM64, with the same `/data`, credentials and port settings.
Omit the Homebrew volume. Source builds select `--target runtime-slim`.
Select individual agent runtimes with the [shared OCI Mod loader](docker/mod-loader/README.md)
and the [Mod Compose example](docker/mods/compose.example.yml). Mods require
explicit root initialization and a writable container layer; the loader drops
to UID/GID 1993 before the original Lapis entrypoint.

The default image includes Homebrew and four agent CLI distributions, which
account for most of its size. Slim loads selected agent runtimes through `DOCKER_MODS`; Homebrew startup
package requests still require the full image. Both images
use Ubuntu's glibc ABI, matching the Rust executable and supported Linux tools.
Alpine's musl ABI requires a separate native build and cannot run the bundled
Homebrew toolchain as a drop-in replacement.

## Agent tooling

The image includes Homebrew plus native `codex`, `cursor-agent`, `claude`, and
`opencode` executables. Cursor's ACP server is `cursor-agent acp`. The tools do
not add agent targets or credentials automatically; mount an operator-owned
`LAPIS_SERVER_AGENTS_CONFIG` and provide each provider's authentication through
its normal environment or files under the persistent `/data` home directory.

The `lapis-homebrew` named volume is populated from the image when first
created, then retains installed packages and versions across container and
image updates. An empty bind mount masks the bundled installation and is
rejected. To add packages at startup, use whitespace- or newline-separated
values:

```sh
export LAPIS_SERVER_HOMEBREW_PACKAGES="jq yq"
export LAPIS_SERVER_HOMEBREW_CASKS="codex"
```

Formula installs are bottle-only because the runtime image intentionally omits
a compiler toolchain. An unavailable bottle, invalid package name, or failed
install prevents the server from starting. Homebrew does not update
automatically; update the persistent installation explicitly with
`docker compose exec lapis-server brew update` and
`docker compose exec lapis-server brew upgrade`.

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

## Content API, MCP and CLI preview

Open a vault, then choose **Settings → Core plugins → Content API and MCP**. Enable
access, select the vaults a client may read, choose a label and lifetime, and
create a credential. Copy it immediately into your client's protected credential
file; the screen clears it after one minute or when dismissed. It cannot be
retrieved again. Each credential is read-only and expires within 24 hours.

The same server process exposes `/api/content/v1` and `/mcp` at the addresses
shown in settings. A new profile starts with access disabled. The enable
preference is saved in the server profile, but credentials become invalid on
restart. Revocation, disabling access, and closing/archiving a selected vault
also invalidate credentials. A token-only server keeps its enable preference
only until restart. Use the owner controls at `/api/v3/content-access` in that
mode; content credentials cannot authorize owner controls.

Set `LAPIS_SERVER_ORIGIN` to the exact public HTTPS origin, or HTTP on loopback.
A reverse proxy must preserve that public Host and forward `/api/content/v1`
and `/mcp`. Other HTTP origins leave content access unavailable. Existing owner
login and CSRF checks protect the settings controls; owner passwords, cookies
and owner-wide bearer tokens cannot substitute for a scoped content credential.

Build the source CLI with `cargo build -p lapis-cli --locked`, then use the
resulting `lapis` executable:

```sh
lapis --target https://notes.example --token-file "$LAPIS_TOKEN_FILE" vault list --json
lapis --target https://notes.example --token-file "$LAPIS_TOKEN_FILE" note read --vault <UUID> --path Note.md
lapis --target https://notes.example --token-file "$LAPIS_TOKEN_FILE" mcp serve --stdio
```

The token file must be owned by your user and readable only by that user on
Unix. Direct HTTP clients send `Authorization: Bearer …` and
`x-lapis-content-api: 1`. Direct MCP uses stateless Streamable HTTP revision
`2026-07-28`; the CLI bridge also accepts initialized `2025-11-25` stdio clients.
OAuth, general MCP client-product compatibility, content writes and packaged CLI
installation remain pending. See the [CLI guide](crates/lapis-cli/README.md),
[HTTP routes](crates/lapis-content-http/README.md) and
[MCP protocol guide](crates/lapis-content-mcp/README.md).

Owner control requests use `x-lapis-protocol: 3` plus the existing owner bearer
or browser session/CSRF credentials:

| Method and path                                   | Request                                                          | Result                                                       |
| ------------------------------------------------- | ---------------------------------------------------------------- | ------------------------------------------------------------ |
| `GET /api/v3/content-access`                      | None                                                             | Availability, addresses, enablement and credential summaries |
| `POST /api/v3/content-access`                     | `{"enabled":true}` or `{"enabled":false}`                        | Updated status; disabling revokes all grants                 |
| `POST /api/v3/content-access/grants`              | `{"label":"CLI","vault_ids":["<UUID>"],"lifetime_seconds":3600}` | One-time `grant_id` and `token`                              |
| `DELETE /api/v3/content-access/grants/<grant_id>` | None                                                             | Revocation, with no response body                            |

Issuance accepts 1–20 distinct active vault UUIDs and 1–86400 seconds. Labels
must be nonempty after trimming, contain no control characters and fit in
128 UTF-8 bytes. A lost issuance response requires checking the summaries and
revoking the inaccessible grant before deliberately issuing another; do not
blindly retry.

## Develop

```sh
pnpm install
pnpm storybook
```

`pnpm spec:check` and `pnpm format:check` check the specification.
`pnpm configure` checks the package manifests.

`pnpm protocol:generate` exports the native Rust contracts to committed JSON
Schema and TypeScript. `pnpm protocol:check` verifies those artifacts without
rewriting them and checks representative clients; CI runs the same gate.
The [content automation plan](spec/src/backlog/content-automation.md) tracks
the shared content API, MCP and CLI work. Its contracts and read-only native
sessions power the optional hosted endpoints described above. Desktop integration,
content writes, OAuth and CLI packaging remain separate delivery gates.
