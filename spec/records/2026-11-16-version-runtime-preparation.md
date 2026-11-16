# Server version-runtime preparation

Source baseline: `62bcbaea3e50c7c05af919f6fa5d7770026f54a4`.

SRV-GOV-001/003, App ownership, private version/tag behavior and the existing
native, release and image gates remain binding. This record distinguishes
functional intake from audit clearance and whole-recipe qualification.

## Supported security intake

Fresh Node24.15.0/pnpm10.34.5 audit initially reported three LOW/six MODERATE
findings over1007 dependencies; production had two LOW/six MODERATE over772.
Compatible registry resolutions refresh Hono4.13.14 to4.13.18 and
node-server2.1.5 to2.1.8, removing all six MODERATE findings without a manifest
range, override, source package patch or runtime change.

Released Mira editor/plugin-mermaid0.0.5 fit Markdown0.3.1's existing `~0.0.4`
ranges. Their normal lock refresh brings Mermaid12.3.0 and browser KaTeX0.18.10,
removing the old Mermaid11 renderer and external Mermaid layout-elk package.
Mermaid12 retains its own elkjs dependency. Two obsolete FastDOM optimizer
entries are removed from this consumer's configuration because that released
plugin no longer depends on FastDOM; remaining entries must resolve against
the installed public package graph.

The rebuilt browser KaTeX chunk has SHA256
`17541902d4510a0967e60f27f6c723bfaea88849a82256a424aed1843ffa1142`;
no built JavaScript contains version0.16.47. The new real-Rust-host acceptance
saves Mermaid math, renders visible SVG/MathML, signs out, restarts the host,
and restores the same saved bytes and diagram through the owned workspace.

## Unresolved audit boundaries

After intake the full audit reports two LOW/zero MODERATE over1008 dependencies;
production reports one LOW/zero MODERATE over773. Neither remaining finding
is waived, and this preparation does not establish deployable audit clearance.

Production GHSA-238p-pmpm-9mq7 remains at KaTeX0.16.47 through
`@lapis-notes/language-service > markdownlint > micromark-extension-math`.
Patched KaTeX starts at0.18.2, but math3.1.0 admits only `^0.16.0`; it is still
the latest released math package. Even markdownlint0.42.1 retains that math
version, while Language Service0.1.6 admits markdownlint`^0.40.0`.
Notes owns this source boundary; its owner was informed without source edits.
No supported registry refresh currently clears it. The actual Language Service
lint/applyFixes call path rendered none of three tested inline/block/trust
math inputs: a bounded diagnostic only. The math root ESM reexport still loads
KaTeX, so this does not establish absence or authorize an audit exemption.

Development GHSA-gcqx-w48m-w2h4 remains through the Svelte Vite plugin and
deepmerge4.3.1. Even the latest plugin still admits that unfixed dependency.
Any exact development exception needs a fresh reviewed binding and separate
operator decision; no other repository's exception is inherited here.

## Before/after acceptance

The optimizer regression initially tested51 inputs:49 passed and the two stale
FastDOM entries failed. After removing those entries, all49 retained inputs
pass. An earlier test-bootstrap attempt incorrectly treated a Svelte-only root
as a CJS export; locating intermediate public package metadata corrected that
bootstrap before recording the49/2 regression boundary.

The original renderer cohort and refreshed cohort each passed the new exact
Mermaid cold/restart case1/1. This proves functional intake, not a security
exploit reproduction. The final refreshed run also passes1/1.

An early refreshed Storybook run had42 passing tests and one import-failed
suite. The unchanged cold ServerHost family subsequently passed68/68 before
optimizer cleanup and68/68 after it: no deterministic import-error cause is
claimed. After cleanup the complete interaction/accessibility lane passes76/76,
and static acceptance passes5/5. No visual baseline was updated.

Final source lanes pass: check (zero Svelte warnings and App ownership),
211 unit tests plus six CLI/workflow tests, web build, Storybook build,
Clippy with denied warnings, and953 Rust tests (11 pre-existing explicit
ignored cases). Startup budget and the actual HTTP/MCP/CLI plugin-operation
case pass under their unchanged limits. Configure, eight demo cases and protocol
parity pass. Formatter, specification check and actionlint pass; semantic QMD
search is healthy. Doctor reports10 passes and the existing custom-alias
warning, not a warning-free result. Ignored model/provider/process helpers
are not claimed as live qualification. Existing frozen Tasks0.1.1/API0.6.5
and peer warnings are retained; newer registry releases are not silently used.

## Browser environment failure and isolated replay

The first full native hosted run stopped at40 passed/one failed/five explicit
existing skips; static acceptance had not run. The failure was
`rust-server-tasks-recovery.spec.ts`, another App preserving interrupted native
Task references. Its trace shows14 loopback `ERR_NETWORK_CHANGED` asset errors
before `openOther` reached a host state and before any Task submission. Cleanup
then exhausted the unchanged120-second test limit. No Tasks product defect is
inferred and no assertion, timeout, fixture or retry was changed.

An ephemeral worker reconstructs the exact tracked source and frozen lock,
compiles its own Rust binary, and runs browser/server in one namespace as the
existing nonroot node user with all capabilities dropped, four CPUs,12GiB RAM
and2GiB shared memory. No host checkout, ABI binary, credentials or socket is
mounted; no image is published. Its tool stage pins official Rust1.99.0 and the
source-declared Node24.15.0/pnpm10.34.5/Playwright1.61.1 contract. Receipts bind
source archive, lock, test/fixture hashes and actual executable/system bytes.
The first environment preflight stopped before tests because the fingerprint
script expected the obsolete headless-shell basename; correcting that local
setup to the actual official executable preserves the original setup failure.
A subsequent web-build setup attempt exhausted the container-default V8 heap
(about2GiB), before tests. The unchanged12GiB worker now explicitly supplies
`NODE_OPTIONS=--max-old-space-size=4096`, matching the passing host default;
the original134 exit and prior receipt are preserved. No source or browser
flags changed. The exact original recovery case passes1/1 in22.3 seconds.
The complete default hosted lane then passes42/42 with five existing explicit
skips, including all15 Tasks cases and the final static case. Metadata's two
opt-in cases remain skipped; properties has two passes. This corrects an early
hand-summed41-pass count for the original host run: its raw results total40.
The original host failure remains recorded; isolated acceptance does not prove
a general network root-cause fix.

The functional snapshot is `791bb6421f08ab9f3057855f6d175805b6c025f1` with
lock SHA256 `4cb358ef546cc37fc4b1d00c89d8d897d5bf111e2e3f3dcb1a7eea1ece426b8f`.
The exact local tool-image ID is
`sha256:75d7d02c4b4ad61a3ba54ce314524e3534c6aa294876b82c3b802ce010614b0d`;
its official Rust stage is pinned to
`rust:1.99.0-slim-bookworm@sha256:2c3a22f0a5533ea2dd5a16627bc841228151faa2d4de2644ac9987e4a2f1f2fa`.
Fresh native binary SHA256 is
`8409d3c2c2c1bbf567edf353ee39629f53b1d5bfc23f43b76501622abe9f358e`.
Browser revision1228 is Chromium149.0.7827.55; receipts retain both executable
hashes, actual compiler/linker/system-library fingerprints and package versions.
This is local functional acceptance; hosted dual-architecture image and
publication gates remain independently required.

## Actual version recipe and remaining work

The source-owned command remains `changeset version && pnpm install
--lockfile-only && prettier package.json CHANGELOG.md --write` under Node24,
pnpm10.34.5 and Changesets3.0.3. The private package permits version/tag
preparation. The genuine GitHub changelog commit lookup must be exercised:
untracked historical input alone does not prove that branch. The pending
`server-http-renderer-security.md` patch note describes these genuine security
repairs after functional acceptance; no qualification-only bump is fabricated. Pre-App guard adoption awaits separately reviewed central
whole-recipe proof against committed source. Publication, GitHub release/tag
creation, image/CLI dispatch and live providers stay outside that disposable
proof. Existing native-skills PR62 is unrelated and untouched.
