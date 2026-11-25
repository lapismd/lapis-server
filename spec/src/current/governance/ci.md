# Affected CI and toolchain

The root Rust package selection includes `lapis-plugin-jobs`. Its source changes
select the engine and every dependent Core, Protocol, host and transport crate.
Unknown or workspace-wide changes retain the complete root selection.

## SRV-GOV-005 — Affected CI and source-free toolchain

<a id="srv-gov-005"></a>

**Requirement.** Pull-request CI MUST derive an exact base/source range, treat
both sides of renames as changed, and select complete owning gates. Rust source
MUST select its Cargo package and transitive reverse dependents. Documentation,
release-note and dependency-automation-only changes MUST NOT run the standalone
Rust, OCI Mod or runtime-image suites. Renderer dependency changes MUST retain
real hosted acceptance even when the standalone Rust suite is not selected.

Root Cargo/toolchain inputs, CI selection code, missing history, invalid paths,
unresolved ownership and unknown inputs MUST fail closed to complete Server CI.
The isolated OCI Mods workspace MUST select both native architectures and the
runtime image consumer without selecting the root Rust workspace. Pushes to
`main`, scheduled runs and explicit manual runs MUST execute complete coverage.
A required aggregate MUST reject every failed, cancelled, missing or
unexpectedly skipped selected gate.

Rust caches MUST bind the toolchain and locked dependency graph, restore safely
on selected pull requests, write only after successful canonical-main work, and
never save failed or cancelled outputs. Selected Rust commands MUST deny Clippy
warnings and test only the affected packages plus reverse dependents. Cache hits
and misses do not replace tests.

The Server CI image MUST be `ghcr.io/lapismd/lapis-server-ci`, extend the
immutable shared `ghcr.io/lapismd/lapis-ci` toolchain, and publish native
`linux/amd64` and `linux/arm64` manifests with provenance and an SBOM. It MUST
pin Node, pnpm, Playwright/Chromium, mdBook, Rust, Clippy/rustfmt, cargo-nextest,
jj and native build libraries; contain no repository source, credentials,
dependency graph, `node_modules`, Cargo registry or compiled target output; and
be consumed only by immutable digest after publication. Docker/Compose, OCI Mod
and runtime-image qualification MUST remain on host runners.

The protocol job checks the generated OpenAPI catalog, including references and documented routes.

### Acceptance details

- PR104-shaped changes select only governance, tooling and renderer acceptance.
- A leaf Rust crate selects its complete Cargo reverse closure.
- Shared or unknown inputs select every Server CI gate.
- Initial CI-image publication remains a separate protected handoff.
