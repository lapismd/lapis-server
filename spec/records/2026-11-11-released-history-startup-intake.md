# Released History startup intake

## Public package and release identity

The owning History fix merged as `785db1d`; version PR 74 merged as
`42376f79f1eae0ba9650a67e4b44c7a6aaf79256`. Exact-main CI run 42987307775
passed. History-only release run 42989480178 passed its full source, browser,
artifact and signing gates and published `@lapis-notes/history@0.2.4`.
All other publication selectors were false, including asset replacement.
Terminal UI npm publication remains held.

The anonymously downloaded npm tarball passes its published SHA-1 and SHA-512
integrity. Its SHA-256 is
`557ba2a7c55775ffb4b6b00f67278cd7547ef9c52bfd82774f2102022caa2c85`.
The npm Sigstore verifier validates provenance against GitHub Actions' issuer
and `lapismd/lapis-plugins/.github/workflows/release.yml@refs/heads/main` identity;
the verified subject binds those tarball bytes and the expected source commit.

The public `history@0.2.4` plugin archive is 191103 bytes, SHA-256
`988edfa1838239961e8753df47a8212af69371a29ebeefa2c10808f6dbb425f3`.
The source repository's published-coordinate verifier validates its trusted
Ed25519 signature, sorted signed file list, every size/hash, checksum file,
runtime coordinates and Git tag against the same source commit. The release
workflow successfully dispatched the verified History registry submission and
notified dependency consumers. Registry submission PR 92 (Index history 0.2.4) is open; its single submission
file names the same archive, version, source commit and package. Registry
deployment remains separate evidence.
Local public verification receipts live under
`.local/public-mods-acceptance/history-release/`.

## Regression and consumer scope

Actual public Server `28d10a6-slim` fixes the old 609-file-command asset fan-out
but fails the unchanged database budget in three cold runs (100, 101, 100).
Healthy History polling redundantly clears persistent notification records for
transient notices. The owning LN-HIST-019 helper fix removes that write while
retaining the public workspace-host transient dismissal. No Server wrapper,
package patch, public API or request-budget change is introduced.

Server now declares normal registry range `^0.2.4`, resolves the published
integrity and retains Notes API 0.2.7 / Workspace 0.3.5 / UI 0.1.8 /
File Explorer 0.2.3 and their existing Design Core compatibility pins.
The refreshed History package also resolves released Core 0.12.3 and Lucide
1.70.0 instead of its prior 0.7.4 / 1.64.0 inputs.

The cold registry-consumer startup fixture passes on remote Docker context
`omarchy`: **112 command / 75 database requests**, **57 / 55 at readiness**,
600 dormant plugin assets and the original **20-second** post-readiness sample.
No `clearNotification` calls occur. Limits remain fewer than 200 commands and
100 database requests. The copied fixture's 643 frontend files exactly match
the completed Vite build by filename and SHA-256.

That isolated derived fixture uses the actual public 28d10a6 slim backend index
`sha256:73ef8e7fcb95ca933ad1ae5b40c0ecb9ebca0357ba75acf2ee9d1d78431b7d56`
and this checkout's newly built frontend. It qualifies the registry consumer,
not a newly published frontend or Server executable. Only the temporary local
harness changes those fixture inputs. The fixture enables core plugins plus
600 dormant assets; it does not exercise all real plugins active.

## Validation and remaining delivery gates

Vite build, Svelte/type and App-ownership checks pass. All 50 Server Storybook
interaction/accessibility cases, all 107 unit cases in 28 files (one worker),
and all five native hosted UI/content journeys pass. The initial default-worker
unit invocations exited without a test summary; only the explicit serial run
qualifies the unit case count. The complete retained native browser families pass 35 journeys, with the same
five existing opt-ins skipped: hosted 5, server 10, History 2, Properties 2,
Tasks 15 and static delivery 1. Metadata/model, backup/original and official
plugin-install opt-ins retain their original conditions; they are not passing
coverage. No new skips were added. Native browser journeys use the existing local Rust debug fixture and the
newly built registry frontend; they do not qualify a rebuilt native binary or
public image. No existing opt-in, assertion, timeout or quarantine is relaxed.

Required hosted CI, exact-main CI and actual-public-image acceptance remain
open. The new full/slim AMD64+ARM64 image identities, actual slim lifecycle/Mod
checks, public agent integration and cold packaged-frontend budget must be
verified after merge. The existing public-image harness preserves the original
entrypoint, binary and frontend while seeding only test-owned data. Prior
28d10a6 runtime/agent receipts are not transferred to the next image.
