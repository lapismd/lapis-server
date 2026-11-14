# Native plugin operations source qualification

The compiled-provider slice implements SRV-OPS-001–004 across Notes, Tasks and
Server. This record qualifies source builds and actual hosted processes, not a
published npm cohort, container image, Desktop listener or CLI release archive.

## Implemented behavior

- Protocol owns versioned descriptors, input/output schemas, help metadata,
  request identities and typed replies. Generated bindings have a drift gate.
- Core registers immutable, namespaced native providers. Credentials retain
  explicit plugin permissions and descriptor digests. Legacy grants gain no
  plugin authority; current vault, provider and credential admission is checked
  again before execution. Native work shares the existing drain lifetime.
- HTTP, MCP and CLI use generic discovery and dispatch. MCP writes require a
  caller-chosen invocation UUID. CLI arguments and bounded JSON input files are
  data; writes are never replayed automatically. Tasks Markdown help is bundled
  for offline command-path and keyword help.
- The Tasks-owned adapter contributes 17 operations over existing native reads,
  guarded transforms and lifecycle journal submission/inspection/retry. Its
  `a79345638076333b8e093795a762416f4d57b9ff` Git revision also supplies the pure
  engine dependency. Workspace patches keep one Core/Protocol type identity.
- Hosted settings offer explicit plugin permissions. Per-vault owner-approved
  Tasks configuration persists outside vault content and restores before the
  listener starts. Explicit disable persists; credentials remain process-local.

## Validation

`cargo test --workspace --locked` and strict workspace/all-target Clippy pass.
The registry cases prove two-provider dispatch, exact scope, old-grant refusal,
foreign-vault refusal, schema/version rejection and provider disable/revoke.
Official MCP SDK and real HTTP acceptance also exercise a separate example
provider without transport-specific dispatch. An HTTP authentication-failure
correlation regression failed once before the fix and passes afterward.

`pnpm test:plugin-operations` builds the real server and CLI and passes with an
isolated native profile. It proves owner configuration, all 17 descriptors,
bounded Task listing, read/write permission refusal, foreign-vault refusal,
saved-version parity, guarded completion, duplicate identity handling through
HTTP/MCP/CLI, explicit inspection and JSON input files. After a graceful process
restart, a new credential observes the same saved version and completed journal
receipt without a browser configuration call; the previous credential is refused.
Task listing also remains available after that cold restart.

Server check, 132 unit cases, production build, all 58 Storybook interactions,
Storybook static build and four static acceptance cases pass. Four catalog
assertions and three hosted assertions initially used the previous read-only
labels and were corrected for scoped access. A fresh Storybook rerun passes
after one module-fetch startup failure. Earlier hosted attempts hit a development
timeout and an intermittent blank sign-in page; focused traced reruns pass. The
final complete `pnpm test:hosted` run passes 39 cases with five existing opt-in
skips. All five startup/content-settings cases and all 15 Tasks cases pass,
including dirty-editor fences, guarded leave, lost-response recovery and a vault
switch that preserves native service authority. No new operation case is skipped.

Permission-option loading had a separate initial-refresh race: its regression
failed once, then all seven model cases and the full unit/catalog/hosted suites
passed after fetching advertised choices independently of cached status.

Notes source API/package checks, type contracts, package validation, root
check/test/build and 276 Storybook cases pass at
`4a63f97550cc2a21a6cc6c32ee85cec9fa3a792f`. Tasks Docker root check/test/build,
package validation, strict provider checks, both native provider cases, generated
catalog drift, specification checks and runner tests pass. Tasks retains its
Docker report identities in `docs/operations.md`.

Protocol drift, specification validation/spec-first, ownership audits and the
spec-validator doctor pass. The doctor retains the existing script-alias warning.
Visual baseline approval remains separate; no baseline was replaced.
The checkout startup budget passes at 69 native commands and 75 database calls.

## Remaining owning work

The content automation backlog retains Desktop integration, installable native
provider execution, OAuth, CLI distribution and later Task creation/title/body
operations. Notes owns package publication and consumers' adoption of its new
public operations port. No package, image or binary archive was published by
this qualification. Provider integration instructions are in
`docs/plugin-operations.md`.
