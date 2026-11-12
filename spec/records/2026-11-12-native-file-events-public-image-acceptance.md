# Native file-events public image acceptance

## Qualified source and public identity

Source `ad7cdd1cc41e8c372a56b54265db5f5375d8d9dd` passed all eight jobs in
[exact-main CI 43189565187](https://github.com/lapismd/lapis-server/actions/runs/43189565187)
and completed [image release 43189565262](https://github.com/lapismd/lapis-server/actions/runs/43189565262).
Both native AMD64 and ARM64 image-tooling and Mod qualification lanes passed.
The merged tree matches the previously qualified feature head. No npm package
was published or republished during this Server image acceptance.

Anonymous GHCR reads verified the index and child manifest digests, plus both
Server configuration digests and exact source revision labels. Normal tag
`ghcr.io/lapismd/lapis-server:ad7cdd1` and compatibility tag `ad7cdd1-slim`
resolve to the same index:

`sha256:18ed79dec67064effbf4f30c122a5bd7d2c82760a992d12dc41e1cded3b9c470`

| Platform    | Manifest digest                                                           | Compressed layer bytes | Unpacked layer bytes |
| ----------- | ------------------------------------------------------------------------- | ---------------------: | -------------------: |
| Linux AMD64 | `sha256:cfcb71330aabad6ca190dc3a8607f0fb89496d18f2982c26c0ca586c9e6bd164` |            120,547,577 |          376,233,984 |
| Linux ARM64 | `sha256:90fb742547aba834286d773787aebfe147863ab2b8aa2f1b1bae109b6470bc4a` |            117,080,915 |          379,932,672 |

Both platform images were pulled through remote Docker context `omarchy`.
Unpacked sizes are the sum of actual image-history layer bytes, preserving the
unchanged 500 MiB gate. Runtime and browser execution below used actual public
AMD64; post-publication ARM64 execution is not claimed. A main-commit release
does not update the version-release-only Server `latest` alias.

## Original runtime and public agent Mods

The unchanged `scripts/test-image-slim.sh` passed against the immutable public
Server index above. It verifies jj 0.45.1 with real init/status and clone/status,
absent Homebrew and bundled agents, the original service and static application,
read-only no-Mod startup, real OCI executable injection and init/post hooks,
application PID 1 UID 1993, graceful exit 0, legacy Homebrew-request rejection,
and the original size limit.

The public loader index is
`sha256:8f7f425008c6e2ac4475c13c6e836957a3ceabe327a4b24d3d4b2a0302d98b6b`.
All four agent indexes were anonymously verified for AMD64 and ARM64:

| Agent    | CLI version          | Public Mod index digest                                                   |
| -------- | -------------------- | ------------------------------------------------------------------------- |
| Codex    | `0.167.0`            | `sha256:7a49616f5d26a18a5327d664a8906c503642e63a4567f8efe9bbbcfcd61232f6` |
| Cursor   | `2026.11.05-86047df` | `sha256:6c6da69a6dac3bae55cd91d3ed65eee092c7d710517906d5b8e50d8aa1343d5b` |
| Claude   | `2.1.311`            | `sha256:7e2df7074bce104681ad39b49fd3b96d7543d2e8ff86a743549cfdc70508a06f` |
| OpenCode | `2.2.6`              | `sha256:1c695360ca878b02b165cbc11be4181bcfac3769fd27b0dcf068d93b56720ec3` |

For each agent, `<agent>-<version>`, `<agent>-ad7cdd1` and
`<agent>-<version>-ad7cdd1` resolve to the same qualified Mod index. All four
immutable public Mod references were loaded together into the actual Server,
first online with an empty Mod cache, then in a fresh container with networking
disabled and the populated independent cache. Both phases report the exact CLI
versions above as UID 1993, retain the original service entrypoint, and stop
with exit 0. The offline phase reports cache hits for all requested payloads.
Mod initialization explicitly starts as root and drops the application to its
original unprivileged identity; no privilege escalation is attempted.

## Cold packaged frontend and native file events

Both browser fixtures use the immutable public image, check its exact revision,
and seed only fresh task-owned data volumes. They preserve the original image
entrypoint, server executable and `/app/static`; checkout build output is not
injected into the application.

The original core startup fixture retains **600 dormant plugin assets**, a
**20-second post-readiness sample**, and strict limits of **fewer than 200
commands / 100 database requests**:

- Total: **69 commands / 75 database requests**.
- At readiness: **49 commands / 62 database requests**.
- Persistent `clearNotification` calls: **zero**.

The separate native file-events fixture uses the default core profile plus the
real public Tasks 0.1.1 package in a development-provenance test deployment,
600 dormant plugin assets and 200 registry-cache files. It preserves the
30-second UI assertions, 120-second browser scenario deadline, 20-second startup
sample and subsequent 20-second idle sample. It passes:

- Native file stream connection, with two requests over the complete journey
  and one bounded lease reconnect during idle, within the existing limit of two.
- Zero idle filesystem list/stat requests and zero idle database requests.
- Zero reads into `.lapis/plugin-registry-cache/fixture`.
- Real external create, modify, rename and delete reflected in Explorer/editor.
- Guarded vault departure, sign-out and original service shutdown with exit 0.

The complete broader journey, including external mutations and guarded leave,
records 180 commands / 201 database requests. These totals are distinct from
the core startup budget above. The existing one-second editing-intent poll is
unchanged; quiet file/database observation does not mean zero network traffic.

The same actual-public-image comparison on the older `e69c10d` failed the
original 30-second ready-shell assertion while restoring the workspace layout,
with 447 commands, 1,507 database requests, 356 cache-fixture reads and repeated
configuration-write conflicts. It never reached the native-stream assertion.
That retained failure is startup/cache regression evidence, not a successful
negative control of the stream assertion. No deadline or assertion was loosened.

The Server native watcher remains recursive at the OS boundary, filtering
application state before journaling except for the targeted `.lapis/app.json`
configuration observation. Browser user-file discovery and observation exclude
application-state trees; the configuration owner retains its separate reload.
The [released API intake](2026-11-12-released-api-file-observation-intake.md)
retains public API 0.2.8 provenance, normal registry ranges/frozen lock, the
unchanged dependency cohort and source/native/browser qualification. Historical
source receipts are not relabeled as public-image execution.

## Reproduction and scope

Public browser commands were:

```sh
LAPIS_PUBLIC_TEST_REVISION=ad7cdd1cc41e8c372a56b54265db5f5375d8d9dd \
  node .local/public-image-startup-budget.mjs \
  ghcr.io/lapismd/lapis-server@sha256:18ed79dec67064effbf4f30c122a5bd7d2c82760a992d12dc41e1cded3b9c470
LAPIS_PUBLIC_TEST_REVISION=ad7cdd1cc41e8c372a56b54265db5f5375d8d9dd \
  node .local/public-image-native-file-events.mjs \
  ghcr.io/lapismd/lapis-server@sha256:18ed79dec67064effbf4f30c122a5bd7d2c82760a992d12dc41e1cded3b9c470
```

The task-local SSH wrapper and Node 24 PATH are required for this remote setup.
Receipts and harness hashes are retained in
`.local/public-mods-acceptance/server-ad7cdd1-acceptance.json`, including public
manifests, both platform sizes, CLI aliases, runtime/agent logs, startup metrics
and `.local/public-file-events-FIaq4m/receipt.json` plus its browser trace. The
older failed comparison remains in `.local/public-file-events-0BRFa1/`.

This qualifies the actual packaged core/dormant and default-plus-Tasks fixtures,
not every real plugin, signed-catalog admission, provider, Desktop or live vault.
The existing five hosted and seven native source opt-ins remain distinct from
passing qualification. The production personal vault was not deployed to or
modified. Only task-owned containers, volumes and networks were removed. The
temporary publication follow-up was deleted after the public gates passed.
