# Vault restoration public image acceptance

## Source, publication and identity

SRV-HST-022 is qualified at source `f16da5778fb6e628bc1ada5d246c2291bda290c9` after all eight
[exact-main CI jobs](https://github.com/lapismd/lapis-server/actions/runs/43252153454)
passed. The merge tree equals checked feature head
`542133ffa4adb9c1f65a0c26330393c4e0cd70c3`.
The automatic [image release](https://github.com/lapismd/lapis-server/actions/runs/43252153776)
passed both Mod qualification jobs, loader and agent publication, and the Server
build/publish step. Its single scheduled status read observed final Buildx
cleanup still running; this record does not relabel that observation as a
completed workflow. The actual published artifacts are independently verified below.

Anonymous GHCR reads verified all seven exact-revision image coordinates:
Server normal/slim, loader and four agent Mods. Both Server platform config
digests and source labels match the exact main commit. Full tag `f16da57` and
compatibility tag `f16da57-slim` share this AMD64/ARM64 index:

`sha256:d94ad85ca5bb6b4d3f899ab713989a5b5eddccc33bc05f026cb01bb593104cee`

| Platform    | Manifest digest                                                           | Compressed layer bytes | Unpacked layer bytes |
| ----------- | ------------------------------------------------------------------------- | ---------------------: | -------------------: |
| Linux AMD64 | `sha256:ee2a304b1c964afd40722680e16372abafc8674e370488d52630eba7f275c514` |            120,547,803 |          376,233,984 |
| Linux ARM64 | `sha256:7db5b8398201f71a88f2a2e1f9e9a376e9438bb31db91579a4adbcf5409c579c` |            117,081,743 |          379,932,672 |

Both actual platform images were pulled on remote context `omarchy`. Unpacked
sizes are sums of image-history layer bytes; Docker's containerd reported size
is retained separately. Runtime/browser execution is AMD64. Post-publication
ARM64 execution is not inferred from a successful pull or manifest verification.
A main-commit release retains the version-release-only Server `latest` policy.

## Actual runtime and original startup budget

The unchanged `scripts/test-image-slim.sh` passed against the immutable public
index. It checks jj 0.45.1 using real init/status and clone/status, absent
Homebrew and bundled agents, the original service/static app, read-only no-Mod
startup, real OCI executable and init/post hooks, PID 1 UID 1993, graceful exit 0,
legacy Homebrew-setting rejection and the original 500 MiB limit.

The actual packaged frontend retains **600 dormant plugin assets**, the original
**20-second post-readiness sample**, and strict limits of **fewer than 200 commands
and fewer than 100 database requests**. It measured:

- **69 commands / 77 database requests** overall.
- **49 commands / 70 database requests** at readiness.
- **Zero** persistent `clearNotification` calls.

The raw response diagnostics retain initial authentication, existing-directory
and missing optional-file probes. These are not repeated app-config write
conflicts, and this receipt does not claim every HTTP response was successful.
The existing editing-intent poll remains unchanged. Earlier source-consumer
69/75 measurements retain their separate scope.

## Actual authenticated reload and logout

The separate restoration fixture starts the original public image with a fresh
task-owned volume: default profile, 600 dormant assets and 200 registry-cache
files. It does not inject checkout frontend output, replace `/app/static`, change
the server binary or override the application entrypoint. The 30-second UI,
120-second scenario and 15-second shutdown deadlines remain unchanged.

The real UI saves a note and the fixture verifies saved bytes and layout before
raw reload. Reload restores the ready vault and saved note without a picker
selection, using a different newly admitted native client. The origin/account
preference contains exactly a schema version and vault ID.

Only the old test-owned client is retired. The existing Problems UI reviews the
saved file and explicitly discards its retained claim before guarded logout;
production automatic claim clearing or replay was not introduced. Reload after
logout requires sign-in, and successful normal login preserves the chooser.
The original service then exits with code 0.

The complete journey records 137 commands / 141 database requests and
2 file-stream requests, with 0 cache-fixture reads. Those totals include
both cold sessions, editing, explicit recovery and logout; they are separate
from the single-start core budget above.

The same immutable harness on prior public `ad7cdd1` passes the saved-note/layout
preconditions and fails the unchanged 30-second postreload readiness assertion
at the vault chooser, with only one admitted client. Its retained trace is a
clean public-image negative control. The earlier harness-order attempt stopped
on an absent preference before reload and retains that narrower scope.

## Agent publication, receipts and scope

All four new Mod indexes are anonymously verified for AMD64 and ARM64.
CLI-version aliases and CLI-version-plus-revision aliases resolve to their new
exact-revision indexes: Codex 0.167.0, Cursor 2026.11.05-86047df, Claude 2.1.311
and OpenCode 2.2.6. Earlier agent online/fresh-container-offline runtime
acceptance remains a separate receipt; those unchanged runtime suites were
not replayed for this Server-browser-only change. No npm work was required.

Receipts and harness hashes are retained in
`.local/public-mods-acceptance/server-f16da57-acceptance.json`, including manifest,
size, alias, runtime and startup evidence. The positive browser receipt is
`.local/public-vault-restoration-gMaL5D/receipt.json`, with its trace. The clean
negative is `.local/public-vault-restoration-SpN1ZK/failure.json` and trace.
All test-created containers and data volumes were retired by their owned cleanup.

This qualifies the actual public default/core/dormant fixture, not every real
plugin enabled together. Plugin activation, Desktop, provider and actual-model
coverage remain separate. No production deployment or user-vault change was
performed. The [source record](2026-11-12-hosted-vault-restoration.md) retains
130 unit, 58 interaction/axe, four static and 39 default hosted passes with five
existing exclusions; the three additional provider opt-ins remain skipped.
