# Hosted UI qualification

The server owns authentication, managed vaults, native transport and its
standalone login/chooser. The built public Notes application profile owns the
workspace session; Desktop retains its native lifecycle adapter and uses the
same public workspace shell.

## Temporary public archives

Qualify and commit each source package before final consumer acceptance. Notes
provides `scripts/validation/pack-consumers.mjs`; Design Core provides
`pnpm package:local`. Plugins provides its collected npm/runtime artifacts;
Agent UI and Tasks provide their built public package archives. Tasks source
checks and local archive acceptance stay distinct from its frozen-registry
`release:prepare` gate. Each ignored archive cohort needs `archives.json` entries containing
`name`, `version`, `path` and `sha256`, plus `provenance.json` containing
`purpose: "local-consumer-qualification"`, the exact source `revision`, and the
same `archives` inventory.

Install one coherent cohort through the normal package manager:

```sh
pnpm validation:profile --manifest /path/to/notes/archives.json \
  --manifest /path/to/design-core/archives.json \
  --manifest /path/to/plugins/archives.json \
  --manifest /path/to/agent-ui/archives.json \
  --manifest /path/to/tasks/archives.json
pnpm validate
pnpm spec:check
```

The installer verifies source receipts, SHA-256, package identity, built exports
and admitted semver ranges. Its hook, installed lock and source vector live in
ignored `.local/ui-parity/consumer/`; the tracked registry lock is preserved.
No source aliases or dependency patches implement this intake.
When a registry patch supersedes one member of a source cohort, select unique
package manifests and retain both source receipts; duplicate package entries
fail closed. The final intake separates registry Markdown 0.2.1 from the other
16 Plugins archives.

## Acceptance lanes

`pnpm validate` runs types, App ownership, unit tests, production build,
production-component Storybook interactions/axe, static Storybook, and the
retained real-Rust suites. `pnpm test:hosted` builds the owned Rust binary and
runs focused login/workspace parity, file/Chat/queue/participants, History/backup, metadata, properties, Tasks
and static-host journeys sequentially with isolated test profiles. The existing
provider configuration remains an explicit opt-in; this command does not
collect provider tests. The metadata/Search, backup recovery and original-source
recovery screens are also temporarily opt-in because repeated renderer attempts
completed their assertions but exceeded the Rust host shutdown deadline while
resolving retained editor state. Re-enable metadata/Search with:

```sh
LAPIS_SERVER_METADATA_SCREEN_TEST=1 \
pnpm exec playwright test -c web/playwright.rust-metadata.config.ts
```

Keep this quarantine documented until the retained-editor teardown boundary is
repaired; it does not weaken the screen's assertions. Re-enable backup recovery
with:

```sh
LAPIS_SERVER_BACKUP_SCREEN_TEST=1 \
pnpm exec playwright test -c web/playwright.rust-history.config.ts \
  web/e2e/rust-server-backup.spec.ts
```

Re-enable original-source recovery with:

```sh
LAPIS_SERVER_ORIGINAL_SCREEN_TEST=1 \
pnpm exec playwright test -c web/playwright.rust-history.config.ts \
  web/e2e/rust-server-original.spec.ts
```

`pnpm test:hosted-ui` covers built preview and a fresh Vite development process
with forced dependency optimization against the real Rust backend.
It verifies branding, centering, system appearance, keyboard entry, actual shell
height, persisted note bytes, Settings, logout and cold layout restoration.
It retains review screenshots. The static-host suite independently serves built
assets from the real Rust binary without Vite.
Storybook checks desktop/narrow and light/dark with real shell bounds. Docs
use isolated 700px embeds; standalone canvases fill their viewport.
`pnpm test:storybook:static` serves only the built catalog and checks login,
desktop/narrow dark shells and the actual 700px Docs embeds inside Storybook's
preview frame. It is included in normal validation and CI.

Host attachment disposal is awaited after database/session closure and before
the application compatibility lease is released. Rust transport implementation
and ownership remain in the server adapter.

Archive acceptance does not qualify a frozen registry install or container
image. Publish the separately authorized source releases, raise the consumer
minimums to those qualified releases and refresh the registry lock before that
independent distribution gate. CI contains the required renderer/hosted lanes;
its frozen install remains pending until those release inputs exist.

Visual review remains pending. Do not update screenshot baselines as part of
functional qualification.
