# Lapis hosted server

The hosted application runs the shared Rust services and the existing Web UI in
one process. Node and pnpm are build tools. The runtime image contains neither
Node nor Deno. The initial operator profile is single-user with multiple vaults.

## Local compiled startup

For a host-native executable, explicitly run
`LAPIS_NOTES_VALIDATION_RUNTIME=native pnpm build:server` from Notes. It builds the native release binary
and the Web application with native metadata, History and Tasks selected together.
Ordinary builds and tests use the repository's Docker-first validation
controller. Those isolated Linux outputs do not become a host-platform binary;
use the explicit native build above for compiled local startup.
`pnpm start:server` launches the compiled output directly; it does not build it.

Set these explicit operator inputs before starting:

| Variable                               | Purpose                                                                               |
| -------------------------------------- | ------------------------------------------------------------------------------------- |
| `LAPIS_SERVER_CONFIG`                  | Absolute path to JSON such as `{"vaults":[]}`; the chooser can create managed vaults. |
| `LAPIS_SERVER_DATA_ROOT`               | Absolute persistent private profile/data directory.                                   |
| `LAPIS_SERVER_ORIGIN`                  | Exact browser origin, for example `http://localhost:8787`.                            |
| `LAPIS_SERVER_BOOTSTRAP_USERNAME`      | Initial owner name.                                                                   |
| `LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE` | Private readable file containing the initial password.                                |
| `LAPIS_SERVER_INSECURE_COOKIE`         | `1` only for local HTTP; use secure cookies behind production HTTPS.                  |

Bootstrap is idempotent: an existing owner/profile is retained. Configuration,
credentials and writable data must remain outside the public Web build. Optional
`LAPIS_SERVER_AGENTS_CONFIG` selects trusted native agent targets and their
declared default modes. Provider executables/credentials are operator-owned;
starting or restoring the server does not submit prompts or replay uncertain work.

`LAPIS_SERVER_LISTEN` defaults to `127.0.0.1:8787`. Relocated compiled startup can
supply `LAPIS_SERVER_BINARY` and `LAPIS_SERVER_STATIC_ROOT`; it still validates the
renderer profile certificate. The executable itself accepts
`lapis-server <config.json> [listen-address]`.

## Container startup

`pnpm dev:server` builds and starts the example Compose profile with explicit
operator config/password mounts. `pnpm build:server:container` exports the exact tracked Notes revision and six
pinned sibling sources, then builds `lapis-server:rust-local`. The bounded Buildx
builder uses the local validation resource settings and keeps reusable caches.
An optional `LAPIS_SERVER_ARCHIVES` manifest qualifies unpublished package
archives by identity and SHA-256 without changing the registry manifest/lock.

The Dockerfile builds from a `lapismd/lapis-notes` checkout. The published
image is `ghcr.io/lapismd/lapis-server`; run it with the root
[`docker-compose.yml`](../../docker-compose.yml) and
[hosted server notes](../../docs/hosted-server.md). It listens on `8787`,
stores the profile in `/data`, and answers readiness at
`http://127.0.0.1:8787/readyz`.

Set `LAPIS_SERVER_CONFIG_FILE` and `LAPIS_SERVER_PASSWORD_FILE` to absolute files,
then run `docker compose up -d` for that public image, or
`docker compose -f crates/lapis-server/compose.example.yaml up -d` for the
local `lapis-server:rust-local` image.
The example uses local HTTP by default. Set its public Origin and cookie setting
appropriately for an HTTPS reverse proxy. It mounts config/secrets read-only,
retains `/data` in a named volume, and supplies temporary `/tmp`. The application
runs as UID/GID1993 with read-only application files. Keep existing volume and
Compose project identities when restarting an established instance.

`pnpm test:server:container` exercises the built runtime image using disposable
owned data and secret volumes: actual identity/runtime checks, authenticated
browser editing and a cold restart with persisted content. It leaves a receipt
under `.local/server-container`. Ordinary source/native/hosted browser validation
remains available through `pnpm test:server` and `pnpm test:server:e2e`.

`/healthz` reports process liveness; `/readyz` reports availability for admission.
A normal stop drains owned work. Unresolved native editing claims can refuse
shutdown and require explicit recovery; do not replace that guard with forced
termination when managing real data.
