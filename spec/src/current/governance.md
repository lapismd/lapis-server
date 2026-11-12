# Governance

<!-- dependency-automation-policy -->

Dependency update PRs from the trusted LapisMD App, Dependabot or Renovate MAY skip specification checks using the PR author rather than the rerun actor. Functional checks remain required. The shared `lapismd/dependency-automation` coordinator proposes published first-party npm updates with Changeset release intent where required; it does not merge, publish, sign or deploy. This CI policy changes no runtime or public contract in this chapter.

The shared session awaits the server-owned Rust attachment/heartbeat close
after database closure and before its compatibility lease is released.
Renderer type coverage includes the production catalog and retained browser
acceptance helpers/configuration. Optional model runtimes load only after their
existing opt-in gate; the complete retained suite still collects without them.
The focused real-server regression retains login and workspace screenshots as
review evidence, independently of visual baseline approval.

The project README documents how to run the image and set the initial owner
password. The checkout demo is governed by SRV-SRV-001 and retains the mounted initial
password behavior in SRV-SRV-003. Demo script regression checks cover preparation,
file preservation, Docker context selection, memory preflight, serial build stages, published image startup and failed-start propagation.
Current contracts and verification own behavior/status; the canonical backlog
owns remaining work and acceptance. Supporting historical receipts retain their
original scope and cannot replace current candidate qualification.

The [content API, MCP and CLI work](../backlog/content-automation.md) adds
portable contracts and a generated-protocol gate to Configure CI. SRV-GOV-001/002
retain their source limits, documentation navigation and host qualification
boundaries. Source contract checks grant no runtime or consumer qualification;
SRV-SRV-005/006 own the additive contract and drift-check acceptance.

SRV-SRV-007 adds scoped read-only native sessions without changing hosted routes,
browser UI, package ownership or release gates. Validation includes focused
native cases, the Rust workspace/Clippy lanes, protocol drift, formatting and
specification checks. Source qualification does not enable external grants.

SRV-SRV-008 adds bounded ephemeral test credentials in shared Core, preserving
the SRV-GOV-001/002 source limits and separate hosted/Desktop acceptance gates.
Focused credential, budget and concurrent drain tests join the existing Rust,
Clippy, protocol, formatting and specification checks. No route or release
automation changes accompany this native credential policy.

SRV-SRV-009 adds the shared content HTTP crate and real-socket adapter tests.
SRV-GOV-001/002 limits and documentation ownership are unchanged. Verification
includes the adapter's focused transport/admission cases, Rust workspace tests,
Clippy, protocol drift, configuration and specification/formatting checks;
production Server/Desktop integration remains separately gated.

SRV-SRV-010 adds the source CLI executable and subprocess acceptance against the
shared HTTP adapter. SRV-GOV-001/002 ownership and release rules remain unchanged:
no binary/image packaging or npm/crate publication is enabled. CLI build, focused
process cases, workspace tests/Clippy, protocol and documentation checks validate
this slice; consumer installation and real host qualification remain pending.

SRV-SRV-011 adds shared MCP reads using the pinned official Rust SDK and shared
HTTP admission. SRV-GOV-001/002 limits, package ownership and release gates remain
unchanged. Official-client and raw HTTP acceptance, revocation during body intake,
capacity/cancellation tests and the workspace/Clippy/protocol/specification lanes
qualify the source adapter only; production host and OAuth gates remain pending.

SRV-SRV-012 adds the source CLI stdio bridge with subprocess interoperability and
shutdown acceptance. SRV-GOV-001/002 source limits, ownership and release gates
remain unchanged. The normal CLI dependency graph remains free of native Core
and host adapters; native MCP/HTTP crates appear only in tests. Both production
host mounts and their plugin settings screens remain separate acceptance gates.

SRV-SRV-013 adds shared native host controls and secret-free status contracts.
Content requirements move to their own chapter within SRV-GOV-001 file limits;
legacy anchors remain as navigation links. Native lifecycle/epoch regressions,
workspace tests/Clippy and generated protocol/specification checks qualify this
source slice. Server/Desktop endpoint and plugin settings acceptance remain open.

SRV-SRV-014 mounts optional content HTTP/MCP in the hosted process with owner
controls and plugin settings through the existing public Notes API. Source,
real-host/cold-process and production UI interaction/accessibility acceptance
remain distinct; Desktop and OAuth are not qualified by hosted evidence.
The hosted Content API/MCP settings use the public Core plugins navigation group;
production stories and cold hosted acceptance verify placement without changing
credential authority or qualifying the pending Desktop host integration.

The [CLI delivery refinement](../../records/2026-11-10-cli-delivery-plan.md)
adds planned Changesets version agreement, Markdown agent help and separate
downloads to the owning backlog. SRV-SRV-010/012 and SRV-GOV-001 implemented
behavior remains unchanged; documentation checks do not qualify CLI distribution.

<!-- Unchanged contract for the participant profile receipt comparison: each delivery is Completed and its outcome is published before the receipts are compared. Validation is `cargo test -p lapis-app-core --test participant_profiles --locked`. -->

Configure preflight rejects a disabled default native projection feature or a
projection dependency that leaves its pinned owning Git source. Source/core
checks and original Desktop acceptance remain separate qualification lanes.

## SRV-GOV-001 — Specification and source limits

<a id="srv-gov-001"></a>

**Requirement.** Authored specification Markdown MUST stay within 400 physical
lines and 32768 UTF-8 bytes. New production Rust and scripts MUST stay within
300 counted lines. Tests MUST stay within 500 counted lines. A protected change
MUST update its owning requirement, or record an unchanged-contract
confirmation with the validation and acceptance review, in the same Jujutsu
change. Notes no longer owns this release gate. Server acceptance and the
image build run in this repository. Agents MUST run `pnpm spec:search` before
a manual scan of `spec/src`, use `--semantic` for a conceptual query, and use
`rg` only when QMD is unavailable. A git tag `v*` MUST publish
`ghcr.io/lapismd/lapis-server` through the image workflow, every push to
`main` MUST publish that image tagged with the 7-character git short hash, and
the published image MUST provide `linux/amd64` and `linux/arm64` manifests.
The image web stage MUST build the Vite application in `web/` after a frozen lockfile install.
Every pull request MUST build and smoke-test the default runtime for
`linux/amd64` and `linux/arm64`, including checksum-pinned jj version and real
repository initialization/status. Those lanes MUST build the native server
stage for both target architectures without running the ARM64 Rust compiler
through QEMU. Both lanes MUST retain the size limit, read-only application,
writable data, Mod lifecycle and obsolete Homebrew request rejection checks.
The reusable OCI Mods crate has isolated registry,
archive and lifecycle regressions; container acceptance proves Mod execution
and unchanged entrypoint signal delivery without Docker socket access.
Changesets MUST record the release note and create that version tag and GitHub
release. This repository MUST NOT publish an npm package. Every pull request MUST run
`cargo test --workspace --locked` with the ordinary Rust dependency cache, and
that command MUST resolve without a Cargo path dependency outside this repository.
The Rust job MUST install checksum-verified upstream jj 0.45.1, then run
`cargo clippy --workspace --all-targets --locked -- -D warnings`, before those tests.

Pages deployments MUST finish before newer runs deploy. One failed deployment
attempt MUST retry the same artifact once within a 25-minute job budget; a second
failure MUST fail the job. Each attempt retains the upstream ten-minute limit
and cancellation handling.

### Acceptance details

- Spec-validator enforces the line and byte limits.
- CI installs mdbook 0.5.4 before the specification check, and Notes CI does not run server acceptance.
- GitHub Pages publishes Storybook at `/lapis-server/storybook/`, including these specification chapters, and does not publish a separate specification site.
- The server mounts published `@lapis-notes/workspace`; pull requests install checksum-verified jj 0.45.1, deny Rust warnings, run cached workspace tests, cross-build the server and smoke-test the Homebrew-free runtime and jj on both architectures; `main` publishes the multi-architecture image through Changesets without npm; and retained Tasks restarts drain guarded sign-out before host shutdown without weakening lost-response recovery.

## SRV-GOV-002 — Published shell styles

<a id="srv-gov-002"></a>

**Requirement.** Storybook MUST import the published `@lapismd/design-core/storybook.css` and `@lapismd/design-core/themes/lapis.css` styles before rendering `WorkspaceShell`. Shell stories MUST fill the viewport with no root padding. The preview MUST default the brand theme to `lapis`, expose the color-mode and theme toggles, and label each specification table-of-contents entry with its requirement id. The hosted application entry MUST keep loading the published design-core styles.

### Acceptance details

- The Storybook preview imports those published stylesheets and defaults `data-ui-theme` to lapis.
- The hosted Notes story requires grid layout, a loaded `--background` token, zero root padding, and full viewport height.
- The centered branded login and managed-vault chooser retain standalone Deno presentation at narrow widths, with workspace chrome appearing after vault opening.
- Each SUMMARY chapter has one Storybook Markdown mirror, and the docs table of contents labels each entry with its requirement id.

## SRV-GOV-003 — Production UI parity

<a id="srv-gov-003"></a>

**Requirement.** The hosted application MUST use the built public Notes application-profile workspace session. Its host wrappers and the real workspace shell MUST fill the viewport. Appearance initialization and branding assets MUST work in application and catalog builds. Host authentication and Rust transport MUST remain server-owned.
The installed dependency graph MUST provide one `@lapis-notes/api` peer context
to the root host and the shared application profile.

### Acceptance details

- Real Rust login, chooser and workspace acceptance verifies viewport bounds, theme and loaded branding; metadata/Search, backup recovery and original-source recovery screens are temporary delivery opt-ins after repeated retained-editor shutdown deadline failures, and the official plugin-install journey is an opt-in while its signed release asset returns 404 in hosted CI, with exact re-enable commands recorded in the hosted UI parity record.
- The shared session retains Settings, configured plugins, guarded persistence and ordered host teardown, while the root package retains its Design Core 0.12.4 compatibility pins for API, application profile, File Explorer, UI and Workspace so the released Notes cohort resolves one API peer context in the frozen registry lock.
- Cold registry-consumer startup acceptance seeds 600 dormant plugin assets and bounds native command/database calls before readiness and over 20 seconds with retained browser/network failure diagnostics and loopback-only host networking for local Linux system-socket fixtures, while chooser navigation retains the live session and final leave owns guarded persistence and teardown.
- Host-owned session handles remain available until asynchronous disposal completes, including Svelte unmount; concurrent browser acceptance awaits guarded departure before destroying each owned App frame.

## SRV-GOV-004 — Production catalog acceptance

<a id="srv-gov-004"></a>

**Requirement.** UI stories MUST mount production components with disposable injected backends. Functional and accessibility checks MUST cover login, chooser actions, startup failure/retry and the ready Notes workspace in light/dark and narrow viewports. Bounds assertions MUST measure the real shell. Visual review remains pending without baseline updates.

### Acceptance details

- Vault-button accessible names include their visible name, role and status text.
- Production-component stories exercise keyboard navigation and the real shell geometry with axe failures enabled.
- Fixtures remain mounted after their play finishes and release resources when the story unmounts.

## Backup diagnostic unchanged-contract confirmation

SRV-GOV-001 remains unchanged for the explicit debug backup stage diagnostics:
source limits, exact Git ownership and public-error privacy remain enforced.
The [initialization receipt](../../records/2026-11-02-backup-initialization-diagnostics.md)
and [shutdown receipt](../../records/2026-11-03-backup-stop-diagnostics.md)
record focused tests, strict Core all-target Clippy and remaining ARM64/hosted
qualification. Diagnostic observability cannot remove the original quarantine,
replace unchanged native assertions or establish a root-cause repair. Fixed
shutdown categories MUST NOT expose native error text, paths, credentials or
vault content.

Terminal admission is implemented in Core and HTTP transport, with the UI imported from the first-party Terminal package. Local qualification may use verified packed UI archives; registry publication and ARM64 image acceptance remain separate release gates.

Native CLI security fixtures create a real POSIX FIFO on macOS and Linux, retaining the same private-token and non-regular-file rejection assertions.

The hosted Vite renderer compiles Terminal public Svelte output directly and prepares Ghostty before first terminal activation, avoiding optimizer-triggered startup reloads.

Native AI cleanup accepts typed 501/UNSUPPORTED when the host has no selected
turn engine. Other failures retain uncertain-cleanup semantics and propagate.
This keeps default-enabled AI compatible with startup Retry on unconfigured hosts.
The reusable Mod qualification caller grants read-only package access explicitly;
its artifact publication remains in the separate image release workflow.

The [command-lease repair](../../records/2026-11-10-vault-backup-lease-release.md)
updates the exact owning Git revision without changing SRV-GOV-001 release,
source-limit or qualification policy. The reproduced Linux failure and owning
regression qualify the dependency intake only; a complete Desktop workflow and
the original macOS ARM64 boundary remain separate requirements. No retry,
deadline, assertion or quarantine is weakened.

Hosted run 42895553521 reached the Specification job and then failed
`pnpm format:check` on `spec/src/verification.md`; its workflow correctly
cancelled downstream jobs after that required failure. The repair compacts
duplicated receipt detail while preserving every requirement id, status,
evidence link and pending qualification. `pnpm format:check` and native
`pnpm spec:check` review this unchanged SRV-GOV-001 contract; product acceptance
remains the separately recorded command-lease evidence.

The [released History intake](../../records/2026-11-11-released-history-startup-intake.md)
retains SRV-GOV-003 registry dependency ownership and cold startup budgets.
History 0.2.4 dismisses transient recovery feedback without persistent database
writes. Source-consumer acceptance and verified npm/archive provenance do not
qualify the next public Server frontend; actual-image identity and acceptance
remain post-merge delivery gates.

The [Core dependency/restart receipt](../../records/2026-11-11-core-update-development-restart.md)
retains SRV-GOV-003 guarded workspace departure and persistent content-access
assertions across restart. Browser fixtures retire the old document after
sign-out and before restarting Vite, so its HMR reconnect reload cannot compete
with the fixture's next cold navigation. Dependency and fixture qualification
do not transfer prior public-image acceptance to a new package cohort.

The [public-image receipt](../../records/2026-11-11-public-image-acceptance.md)
qualifies the immutable 7563116 slim frontend and original runtime on remote
AMD64 with the unchanged startup request budgets, original lifecycle and public
agent Mods online/offline. Full/slim AMD64/ARM64 publication and exact-main CI
are verified separately from post-publication ARM64 execution, all-active-plugin,
provider and Desktop acceptance. Earlier source-only receipts retain their scope.

The [agent version-tag receipt](../../records/2026-11-11-agent-mod-version-tags.md)
adds a CLI-version alias to the same qualified Mod index without changing
payloads or image build inputs. README configuration retains explicit root
initialization, writable Mod application, independent cache and application
privilege drop. Registry publication remains a post-merge verification gate.
