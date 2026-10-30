# Pinned native projection dependency restoration

Desktop's exact `cargo test --test core_tasks_service` boundary failed one of
four tests: Core returned Unsupported because the ordinary Git dependency
compiled `tasks_closed` and `indexing_closed`. Retained engine implementations
were hidden behind a non-default feature with no declared engine dependencies.

App Core now declares the owning plugin, Tasks and Docs native crates at exact
Git commits and enables the existing projection feature by default. No source
code is copied. The historical feature name remains compatible, and explicit
per-vault native ownership, generated-write fencing and provider admission remain
unchanged. The previously gated source tests now run in the default suite.

Source validation passed: `cargo test --workspace --locked` completed 698
tests across 140 suites with zero failures and seven existing model/asset
ignores. `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`pnpm configure`, all three specification lanes and formatting passed. The exact
Core `service_task_writes` regression also passed. Original Desktop acceptance
and hosted CI remain pending. No provider, visual or distribution gate is
promoted.

Initial full Core library run: 131 passed, 1 failed, 1 existing ignored model
case. The two-vault backup History test returned a binding-validation error.
Its exact isolated rerun passed unchanged; the next complete Core library run
passed 132 with the same one ignored case. This failed attempt is retained; no
History assertion, retry rule or timeout was changed. The final full workspace
run above includes the integration suites; hosted CI must pass before merging.
