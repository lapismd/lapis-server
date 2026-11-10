# Local Compose demo — 2026-11-09

The checkout is based on main commit
`45f4c1b482d0dd27b8a8675c49a7e7aea632c4f1`. The root Compose file supports
building this checkout or selecting a published image with `LAPIS_SERVER_IMAGE`.
The demo script prepares ignored local environment/config/password files,
preserves existing files, uses an explicit Docker context, and waits for health.

## Published image acceptance

Command from the repository root:

```sh
LAPIS_DEMO_DOCKER_CONTEXT=desktop-linux ./scripts/demo.sh image ghcr.io/lapismd/lapis-server:45f4c1b
```

- Docker Desktop is local; the `default` context's socket was absent. The user
  selected `desktop-linux` explicitly. No remote daemon was used.
- Image inspection confirms `linux/arm64`, revision
  `45f4c1b482d0dd27b8a8675c49a7e7aea632c4f1`, and repository digest
  `sha256:ec459eec169b06b9b528a12bae7c271c61698039a2c556a3b58cef06d3e1564f`.
- Compose reports the container healthy, bound to `127.0.0.1:8787`.
- `/readyz`, hosted HTML with asset references, owner login using the generated
  password, and the authenticated vault list each returned HTTP 200. The new
  profile has zero vaults; the owner can create one through the application.
- Persistent profile/Homebrew volumes and read-only config/password mounts are
  active. This smoke does not qualify browser interactions or restart persistence.

## Source and tooling checks

`node --test scripts/demo.test.mjs` passes eight cases covering preparation,
preservation, context selection, serial source builds, failed startup, invalid
input, build-memory preflight and published-image startup. Configure CI runs
this lane. `pnpm check`, `pnpm test` (99 tests), `pnpm build`, `pnpm configure`,
formatting, Compose config validation and specification checks passed locally.
The installed `@lapismd/spec-validator` 0.2.0 matches the latest registry release.
Doctor passes with Node 24; Node 26 has an incompatible system QMD native ABI.
The source checks did not require a product-code change.

## Cold source-build limitation

A cold image build encountered a Docker VM read-only filesystem, followed by a
corrupted pnpm cache after restart. User-authorized unused-image/cache cleanup
preserved containers and volumes. A serial Web-only attempt then failed with
`cannot allocate memory` at the original 3 GiB Docker allocation. The script now
rejects insufficient build memory and recommends allocating at least 6 GiB.

A temporary 6 GiB allocation was restored to the original 3072 MiB after host
free disk space fell below 300 MiB and the retry was stopped. The user selected
the published image instead; failed build cache was reclaimed. A complete cold
source-image build remains unqualified. No hosted CI or release was triggered.
