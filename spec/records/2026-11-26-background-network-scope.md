# Background network scope and resolver qualification

SRV-JOB-009–011 extend the shared native runner with explicitly approved
origins/methods and opaque native credential injection. Beancount's production
plan remains the bank integration reference: native Rust owns provider work,
the UI receives opaque references, and reviewed revision-checked ledger imports
remain behind its admitted service. This extension does not implement provider
connectors or bypass that service.

All executable qualification uses Docker context `omarchy`.

## Scope regressions

`cargo test --locked -p lapis-app-core --test service_background_network`
first reported seven passes and two failures on source
`b7dd769e6ec3f23a856fa76576638094e46e039b`. A job namespace could also be a
foreground plugin ID, and cold journal grants could exceed the verified
declaration. The native namespace now contains a plugin-ID-prohibited colon;
every source/broker revalidation checks the stored permission subset.

The exact target then passed nine tests with zero failures on
`59636bf96aaa1437ab2a5c0733091b2002775c9f`. The accompanying network policy
suite passed three tests with zero failures. Original logs remain in the
ignored local qualification receipts and retained remote containers.

## Resolver cancellation

Hickory 0.26.4 replaces Tokio's blocking OS DNS resolver with async sockets.
Fully qualified requests prevent search-suffix widening; vetted addresses
remain pinned in reqwest. Resolver operations share admission polling and the
VM's deadline. A stalled local UDP resolver proves cancellation and deadline
expiry, including prompt runtime teardown.

The first test fixture used `.invalid`, which Hickory rejects locally before
query dispatch; three policy tests passed and the new resolver case failed.
The corrected fixture uses a synthetic name under `example.com` and a local
resolver that never answers. On source
`819067f88781f85b58f810505da1c04cc0f133a8`, both exact commands passed:

- `cargo test -p lapis-app-core --test service_background_network`: nine passed.
- `cargo test --locked -p lapis-app-core --lib services::jobs::network`:
  four passed, including resolver cancellation and the three policy tests.

An earlier successful retry failed only while exporting Cargo.lock to a
read-only input directory. The final export uses the owned report volume.
Lock resolution adds the resolver dependency closure without changing existing
locked package versions.

The first strict workspace Clippy attempt after the fixture field rename
reported one redundant field initializer. After using the shorthand initializer,
the same strict command passed. The full workspace test run remains distinct.
`spec-validator doctor` passed with twenty checks and one script-alias warning;
the QMD `pnpm spec:search SRV-JOB-009` query found the current contract and
verification row.

## Full source qualification

On source `cd60c7d2d642ca042d8e64001793ef855ba0d8a4`,
`cargo clippy --locked --workspace --all-targets -- -D warnings` passed;
`cargo test --locked --workspace` passed 1,029 tests, with zero failures and
twelve existing ignored cases across 206 summaries. `pnpm protocol:check`
also passed. The following specification check rejected prose inside acceptance
details. Its corrected bullets initially exceeded the four-bullet limit; the
final four-bullet form passed validation. These failed commands remain logged.

Source `3808ba29f57e2a86a954e4aec5965a77a8b5ddea` then passed `pnpm spec:check`,
`pnpm format:check`, `pnpm build`, and
`cargo build --locked -p lapis-server -p lapis-cli`. Adding the hosted browser
case initially exposed its MCP Apps chapter mapping; the missing cross-reference
was added before those checks passed.

The first real synthetic-bank browser case stopped at configured-core-plugin
startup before credential replacement or dispatch. Its current registry Notes
graph and retained trace require diagnosis and updated consumer qualification;
this source receipt does not count that failed browser run as host acceptance.

## Remaining gates

Actual host journeys, released Notes controls and Desktop noninteractive native
custody remain required. Credentials, request URLs,
bodies and user profiles are excluded from receipts. No live bank was called.
