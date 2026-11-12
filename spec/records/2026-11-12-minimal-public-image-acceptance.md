# Homebrew-free public image acceptance

## Qualified source and public identity

Source `e69c10d020f4bc70b36b272ddde83923adb9446e` passed exact-main
[Server CI 43124555437](https://github.com/lapismd/lapis-server/actions/runs/43124555437)
and [image release 43124555621](https://github.com/lapismd/lapis-server/actions/runs/43124555621).
Both native AMD64 and ARM64 image-tooling and Mod qualification lanes passed.
No npm publication was part of this delivery.

Anonymous GHCR reads verified the index, child manifest and server config
digests. Both architecture configs identify the exact source above. Normal
`ghcr.io/lapismd/lapis-server:e69c10d` and compatibility `e69c10d-slim` tags
resolve to the same index:

`sha256:988bf01036a7bf2f5028dc55660957e883d92ea469c944ffb4fe0f1fd7afcc3a`

| Platform    | Manifest digest                                                           | Compressed layer bytes |
| ----------- | ------------------------------------------------------------------------- | ---------------------: |
| Linux AMD64 | `sha256:b542e86c933c01057e759ce61f06bbffe12be4019e7a63ae25cede9d0e2cd121` |            120,482,515 |
| Linux ARM64 | `sha256:a34650f23a870b4f860f35703d8829ec9ec40f1c4c92a3eb642182137089df41` |            119,350,857 |

The immutable public index was pulled on remote Docker context `omarchy`.
The actual AMD64 unpacked layer-history total is **376,160,256 bytes**.
Docker's containerd image-inspect size describes stored content; it is not the
unpacked-size measurement used by the unchanged 500 MiB gate.

## Actual runtime and packaged frontend

`scripts/test-image-slim.sh` passed against that public digest. It verifies
jj 0.45.1, real repository initialization/status and local clone/status,
absent Homebrew and agent distributions, original entrypoint/readiness,
read-only no-Mod startup, real OCI executable injection and init/post hooks,
application PID 1 UID 1993, graceful shutdown, unsupported Homebrew request
rejection and the unchanged unpacked-size limit.

The cold public-image browser harness checked the revision and repo digest,
kept the original entrypoint, server binary and packaged frontend, and seeded
only a fresh task-owned data volume through the Docker API. No application
file was replaced or copied from the checkout. The original **fewer than 200
commands / 100 database requests**, **20-second** post-readiness sample and
**600 dormant plugin assets** were retained:

- Total: **112 commands / 77 database requests**.
- At readiness: **57 commands / 65 database requests**.
- File commands: 80; editing intents: 29; text reads: 3.
- Persistent notification clears: zero.

Captured diagnostic responses retain the existing pre-login owner 401,
missing installed-plugin state 404 and idempotent directory-creation 409s.
The request-budget gate passed without changing an assertion or deadline.

This is the core/dormant-asset fixture, not all real plugins enabled. It still
uses the released API 0.2.7 cohort and browser polling. The separately qualified
native file-event integration requires API 0.2.8 publication and a subsequent
Server release; this receipt does not claim that fix is deployed. Workspace
0.3.5, UI 0.1.8, File Explorer 0.2.3 and History 0.2.4 remain unchanged.

## Published loader and agent Mods

The public loader index is
`sha256:de55b2b18459f7c86995f528682f4408d5521ecbd590c661e057fa307475fbd8`.
Anonymous index/child digest checks verify Linux AMD64 and ARM64 for it and
all four agent Mods:

| Agent    | Reported CLI version | Immutable Mod index digest                                                |
| -------- | -------------------- | ------------------------------------------------------------------------- |
| Codex    | 0.167.0              | `sha256:01428dafdd0ecacdb47c77ecffc0a33274bf0eb3d90d8483d87d9f7cbd245e12` |
| Cursor   | 2026.11.05-86047df   | `sha256:c8b7e80e901730faa0255a2ed36ea82985a8da0bc90b0e291d689e6ddf3a1bc2` |
| Claude   | 2.1.311              | `sha256:16408e36d25d940899e8eb68d9097eba5cf6dc383e7a3e9645100dba07486dd2` |
| OpenCode | 2.2.6                | `sha256:bfad490145f838fe1b147d4b5c7e14bafcb9396746dbc1ff141118001bf1d4c3` |

The CLI-version tag, version-and-commit tag and `e69c10d` commit tag were
anonymously verified to resolve to each same qualified index. The actual
public Server digest passed online with all four digest-pinned Mods and again
in a fresh container with networking disabled and the same verified cache.
Each executable reported the version above as UID 1993. Both phases reached
original readiness and stopped gracefully; offline logs record cache hits.

Use the [README Mod instructions](../../README.md#configure-agent-mods) and
[Compose example](../../docker/mods/compose.example.yml) with
`LAPIS_SERVER_TAG=e69c10d`. Mods need explicit root initialization and a writable
container layer; the loader then executes the original application as
UID/GID 1993. Keep existing `/data` and use a separate persistent Mod cache.
The no-Mod default remains rootless and supports a read-only root filesystem.

## Evidence and limits

Receipts under `.local/public-mods-acceptance/` are
`images-e69c10d-manifests.json`, `server-e69c10d-identity.json`,
`server-e69c10d-{pull,slim,agents,budget}.log`,
`mods-e69c10d-references.txt` and `agent-version-aliases-e69c10d.json`.
The cold browser's separate `public-startup-budget-*` directory retains its
identity, metrics and response diagnostics.

All Docker testing used `omarchy` and test-owned containers, volumes and
networks. No production vault, unrelated resource or old Homebrew volume was
modified. Runtime/browser execution here is AMD64; ARM64 native CI and public
identity are verified, but post-publication ARM64 execution is not claimed.
Existing hosted/native opt-ins remain unchanged. Actual provider authentication,
Desktop, production deployment and the live personal vault remain separate
qualification surfaces.
