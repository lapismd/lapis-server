# Public Server and OCI Mod acceptance

## Qualified release identity

Server source commit `75631165c25a0b6cb5e464d93c8314a471ef5160`
contains the released History 0.2.4 repair and frozen Notes API 0.2.7,
Workspace 0.3.5, UI 0.1.8 and File Explorer 0.2.3 cohort.
All eight PR checks passed on 9387a329. Exact-main CI run
[43003377341](https://github.com/lapismd/lapis-server/actions/runs/43003377341)
and image release run
[43003376945](https://github.com/lapismd/lapis-server/actions/runs/43003376945)
completed successfully on the qualified source commit.

Anonymous GHCR requests verify each index and child manifest digest. Both
server architecture configs carry the exact source revision label.

| Public image                                | Immutable index digest                                                    |
| ------------------------------------------- | ------------------------------------------------------------------------- |
| `ghcr.io/lapismd/lapis-server:7563116-slim` | `sha256:cd371f1c40b3badc3bb69a1a904c2f30867f968472016c2b632f27a197c205af` |
| `ghcr.io/lapismd/lapis-server:7563116`      | `sha256:232d10bece68fea949b6e4527400a9b3605f1fd8c124c772b34c2611ab09eb49` |

Both indexes contain Linux AMD64 and ARM64. Compressed layer totals are
85,993,048 / 85,250,940 bytes for slim AMD64 / ARM64 and
947,514,821 / 938,248,285 bytes for full AMD64 / ARM64. Actual pulls on remote
Docker context `omarchy` verify the revision and repo digest; unpacked history
sizes are **262,922,240 bytes slim** and **2,355,445,760 bytes full** on AMD64.

The full image retains its Homebrew prefix and the five existing CLIs for
compatibility. The slim base contains none of those tools, Node or Deno.
Agent Mods provide selected runtimes without Homebrew. Those optional downloads
and their writable/cache space are additional to the slim base size.

## Actual packaged frontend and runtime

The actual public slim index above passes `scripts/test-image-slim.sh` on
`omarchy`: original startup, real OCI executable injection, init/post hooks,
PID 1 UID 1993, read-only no-Mod startup, graceful stop and the unchanged
500 MiB base limit. The pulled full image also passes its non-root filesystem
and bundled-CLI layout checks; a full-image lifecycle/browser run was not added.

The cold browser harness pulls that immutable slim image and checks its revision
and repo digest before startup. It retains the ORIGINAL entrypoint, executable
and packaged frontend. Only a fresh test-owned data volume is seeded through
the Docker API; no application file is copied into the image.

The original **fewer than 200 commands / 100 database calls**, **20-second**
post-readiness sample and **600 dormant plugin assets** are unchanged:

- Total: **112 command requests / 77 database requests**.
- At readiness: **57 commands / 68 database requests**.
- File commands: 80; editing intents: 29; text reads: 3.
- Persistent notification clears: **zero**.

The old public 45b3b0c slim negative control failed at 609 file commands before
readiness. Public 28d10a6 slim removed asset fan-out but still failed at 100,
101 and 100 database calls from redundant History feedback clears. The source
fix and released dependency intake are described in the
[History intake receipt](2026-11-11-released-history-startup-intake.md).
The current result qualifies the actual public frontend, not a checkout build.

Scope: fresh core-plugin/dormant-asset fixture, not all real plugins active.
Runtime acceptance here is AMD64 on `omarchy`. ARM64 source/artifact qualification
passed native CI and public ARM64 manifests/configs were verified, but a new
post-publication ARM64 runtime/browser run was not performed. Existing five
hosted opt-ins and the separate provider/Desktop qualification remain unchanged.

## Published loader and four agent Mods

The actual loader index is
`sha256:2a5412cbaeec7bda732f4cb3aeb8d29eb3534aa8952c01388c10d3e9f17f6e97`.
Anonymous multiarchitecture digest verification also passes for every
`ghcr.io/lapismd/mods:<agent>-7563116`:

| Agent    | Immutable index digest                                                    |
| -------- | ------------------------------------------------------------------------- |
| Codex    | `sha256:2b8bed934b1b33a9457f0f255ca71e26af37b846ad90d4af88f8474c4c7b3c4a` |
| Cursor   | `sha256:939ff9366be1d558b84883ce94d1d44cdfbc4c80d18a40a9f0b61252c0a2c72b` |
| Claude   | `sha256:bccb36150477de20ecf3564c87b415b848584b27ac65495706bc78a3d6510f0c` |
| OpenCode | `sha256:e52c9c2a69a89fc0062f4cc18c60d7f2390fe05f9b820446ff2bf0b463124dce` |

Actual published loader/agent artifacts pass online and recreated offline-cache
CLI checks. The actual public slim Server additionally passes with all four
public digest-pinned Mods: original readiness, all four executable version
checks as UID 1993, a new container with networking disabled and verified cache
hits, original application UID and graceful shutdown. Reported versions are
Codex 0.167.0, Cursor 2026.11.05-86047df, Claude 2.1.311 and OpenCode 2.2.6.
No Docker socket or unrelated container/volume was used or altered.

## Use and retained receipts

Use `ghcr.io/lapismd/lapis-server:7563116-slim` as the Compose image, or pin its
index digest above. The root demo supports `pnpm demo image <reference>` with
its existing prepared environment; Compose config/secret bind paths must exist
on the chosen daemon host. The
[agent Compose example](../../docker/mods/compose.example.yml) accepts
`LAPIS_SERVER_TAG=7563116` and pipe-separated `DOCKER_MODS` references.
Choose only the required agents and prefer the immutable digests above.
Its initialization uses UID 0 and a writable container layer, then the loader
executes the unchanged application as UID/GID 1993. A persistent Mod cache
volume is independent of `/data`. The ordinary read-only no-Mod profile remains
supported. Testing used only remote Docker context `omarchy` and test-owned data.

Local receipts under `.local/public-mods-acceptance/` include
`images-7563116-manifests.json`, `server-7563116-acceptance.json`,
`server-7563116-{pull,slim,budget,agents}.log`,
`server-7563116-full-{identity.json,pull.log,layout.log}`,
`loader-agents-7563116.log` and `mods-7563116-references.txt`.
History npm 0.2.4 was separately verified with public integrity, Sigstore
provenance and trusted archive signature/tag evidence under `history-release/`.
Registry submission 92 remains separate from registry deployment. No other npm
publication occurred; Terminal UI npm publication remains explicitly held.

This completes the requested public-image acceptance for 7563116; the release
follow-up is removed after recording these results. No broader hosted/provider,
Desktop or all-active-plugin claim is made.
