# Approved Desktop access for CLI and MCP

The CLI now resolves a private Desktop connection before content, contributed
operation and stdio MCP dispatch. Desktop owns publishing that record only after
native credential export; the shared Core grant remains the sole authority.

## Regression evidence

- Before: `cargo test --locked -p lapis-cli --test reads desktop_discovery_reuses_approved_access_without_login` failed once with INVALID_REQUEST.
- After: the exact regression passes, including revocation and nonprivate-file refusal.
- `cargo test --locked -p lapis-cli`: 30 passed, zero failed, including official SDK stdio discovery/read, unsafe-record refusal, CLI/Core edit preservation, installation and offline help.
- `cargo test --locked -p lapis-cli -p lapis-content-http -p lapis-content-mcp`: the initial widened run passed 50 tests; two new CLI discovery/stdio cases were added afterward and the full CLI suite rerun.
- `cargo clippy --locked -p lapis-cli -p lapis-content-http -p lapis-content-mcp --all-targets -- -D warnings`: passed.
- `cargo build --locked -p lapis-cli`: passed.
- `pnpm spec:check`: passed after adding mapped source chapters and keeping governance within 400 lines. The earlier missing-chapter and 405/402-line attempts failed and are not qualification.

The first full CLI run found one changed no-setup exit code (3 rather than 2).
Discovery now distinguishes missing setup as INVALID_REQUEST, preserving the
original assertion; the same complete suite then passed.

Versions: Rust 1.99.0, Node 26.8.2, pnpm 10.34.5, spec-validator 0.2.1
(latest registry release checked). Both spec-validator doctors passed config,
QMD and native-module checks, with existing custom-alias warnings. Both QMD
semantic searches completed successfully.

## Limits and integration

Desktop's MCP mount uses its existing server revision
`83fca8ddd25b7b7913723e231779862f94f7dbcb`. Its bundled CLI remains built from
that older revision and requires a later verified source intake. Source
acceptance uses the standalone executable built in
`.local/rust-core-target/debug/lapis`; it does not replace an installed command
or application. No package is published and no user credential is accessed.

Native consent and cold packaged Desktop acceptance, Windows custody, durable
approvals after restart, remote OAuth and full content mutations/search remain
unqualified. Local connection discovery is a version-1 secret-free record,
not a new authentication service. Existing explicit remote URL credentials are
unchanged. No tests, assertions or visual baselines were weakened.
