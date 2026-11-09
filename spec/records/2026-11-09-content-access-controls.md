# Shared native content access controls

Date: 2026-11-09. Owning requirement: SRV-SRV-013, with the existing portable
contract/drift gates SRV-SRV-005/006. Jujutsu change `uluqpzny`, based on main
`15d4040c` after the CLI stdio merge, with the unchanged content-contract chapter
extracted into preceding documentation change `kxkrxxpy`. No host endpoint or settings screen is
qualified by this source receipt.

## Behavior and ownership

Core extends its existing grant registry with explicitly disabled startup,
serialized enable/disable/shutdown, named read-only issuance, owner status and
whole-grant retirement by vault. Disabling fences every grant before waiting
for accepted readers. Admission captures an epoch before native validation and
rechecks it before publishing a credential; disable/re-enable and retirement
invalidate pending issuance. Re-enable retries failed drain and cannot reopen
while any failed retirement remains. Shutdown is permanent.

Status contains only grant IDs, labels, admitted vault IDs, read scope, active
flags and remaining monotonic lifetime. It contains no credential/verifier and
performs no native IO under the registry lock. Tokens still come only from the
issuance result, and all authority remains process-local. The existing enabled
`Default` constructor and unnamed low-level issuance remain fixture APIs;
production hosts must use `disabled()` and owner-authorized named issuance.

Hosts must fence native vault admission against retirement, own accepted control
work after request cancellation, schedule expiry cleanup, and keep persistent
consent/listeners outside Core. Shared Protocol adds only status DTOs, not
bearer-callable management operations. Native read/write policy is unchanged.

## Before/after evidence

The new `disabled_startup_and_reenable_never_restore_credentials` regression was
added before implementation. `cargo test -p lapis-app-core --test content_access
--locked` failed compilation with one missing `ContentGrants::disabled` API.
The exact target now passes all three integration cases, with zero failures.

`cargo test -p lapis-app-core --lib services::content::grants --locked` passes
eight lifecycle cases (five new), zero failures. Native lock/epoch synchronization
proves all-grant fencing before disable or vault-retirement drain, serialization
of re-enable, and refusal of issuance started before an intervening transition.
The failure-path test poisons a private session mutex and proves that failed
drain keeps access disabled while unaffected entries still retire. Expiry uses
the original monotonic deadline and remains enforced before cleanup.

`cargo test -p lapis-app-core --test content_grants --test content_reads
--test content_access --locked` passes thirteen integration cases. New coverage
includes startup/re-enable, redacted status, label/UTF-8 bounds, exact scope,
read-only enforcement, permanent shutdown, whole-grant vault revocation and
preservation of unrelated credentials. Existing credential and read suites remain unchanged apart from an explicit
test-only Duration import.

## Wider validation

- `cargo test --workspace --locked`: 768 passed, zero failed, seven existing ignored cases.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `pnpm protocol:generate` and `pnpm protocol:check`: generated status/schema exports and strict TypeScript checks passed; owner controls remain absent from content commands.
- `pnpm configure`, spec-validator doctor and QMD specification search passed; doctor retains the existing script-wrapper warning.
- Changed Rust files pass Rustfmt; `pnpm format:check` and `pnpm spec:check` passed, including mdbook, mirrors/navigation, spec-first and the App ownership audit.
- After refreshing the frozen main lockfile (including its independently merged Design Core 0.12.3 update), `pnpm check` passed with zero errors/warnings and `pnpm test` passed 99 tests.
- `pnpm build`, `pnpm test:storybook` (46 interaction/axe cases), `pnpm build-storybook` and `pnpm test:storybook:static` (four cold built-output cases) passed. No visual baselines changed.

The growing content requirements moved into `current/content.md` to retain
specification file limits. Original server anchors remain as links so historical
receipts stay readable. Navigation, Storybook mirrors and canonical matching
were updated together; historical records were not rewritten.

## Remaining qualification

The hosted server must mount optional HTTP/MCP routes, persist its owner setting,
provide independently owner-authenticated controls and expose the actual plugin
settings screen. Desktop must consume the shared controls/adapters and qualify
its listener, settings screen, vault/window admission and custody. Neither host
is completed here. OAuth/client-product compatibility, durable grants/consent,
content writes and CLI packaging remain in the owning content backlog.
