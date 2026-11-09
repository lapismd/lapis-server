# Content contract foundation

Requirements: SRV-SRV-005 and SRV-SRV-006. Source base: server main `5b79d9d2`.
This is the first implementation increment of the content automation plan.

## Change and boundary

Add portable content request/reply DTOs, semantic validation and operation
scope/effect descriptors to App Protocol. Content API major 1 remains separate
from browser protocol major 3. Creation is absent-only; replacement requires
a strong version. Validation rejects caller-supplied native authority, unsafe
or reserved paths, invalid IDs and oversized text/page/cursor/version inputs.

The protocol exporter includes these DTOs in the existing schema/TypeScript
artifacts. A server-owned generator/checker and CI gate detect drift and
type-check clients without writing to Notes or other repositories.

No content executor, grant, durable receipt, endpoint, CLI or MCP host is enabled.
Catalog entries are contract inventory, not runtime capability advertisements.
Host authentication, physical paths, editor claims and accepted-work drain keep
their existing owners. Notes/desktop adoption and actual transport parity remain
in the owning backlog; this receipt does not qualify them.

## Validation

Environment: Linux, Rust 1.99.0, pnpm's Node 24.19.0, system Jujutsu 0.46.0.
CI separately installs its pinned Jujutsu 0.45.1; local results do not replace CI.

- Baseline `cargo test -p lapis-app-protocol --test wire --locked`: 5 passed.
- Before implementation, the new content target failed to compile on its missing
  `content` module. After implementation, the final focused target passed 8/8.
- `cargo test -p lapis-app-protocol --locked`: 42 passed, 0 failed/ignored.
- Drift negative case: `pnpm protocol:check` failed on both stale artifacts
  after the new Rust contracts were added and before regeneration. Original
  artifacts matched the original exporter before the change.
- `pnpm protocol:generate`, then `pnpm protocol:check`: generated artifacts
  match and strict TypeScript fixtures pass, including rejected unsafe requests.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `pnpm check`: zero errors/warnings and App ownership audit passed.
- `pnpm configure`, `pnpm spec:check`, `pnpm format:check`, and changed-Rust-file
  rustfmt checks passed. An earlier spec attempt rejected multiline/too-many
  acceptance bullets; the requirement prose and atomic acceptance bullets were
  corrected before the passing check.

`cargo test --workspace --locked`: 710 passed, 0 failed, 7 ignored.
The existing ignored cases retain their model-asset or parent-invoked subprocess
roles; this change adds no ignores and does not qualify missing model assets.
