# Hosted qualification and delivery

Runtime replacement is implemented: this repository owns the Rust server/core/
protocol crates, image, static application and hosted acceptance. The old Notes
ledger's statement that runtime cutover is still primary is superseded by its
later landed-main receipts and the server extraction. This does not transfer an
old receipt to a different source, dependency or image cohort.

## Coherent release-input qualification

Disposition: qualification required for each release candidate. Retain the
original boundary: supported authenticated hosted journeys, multiple managed
vaults/browser sessions, file/database/History/Search/Tasks, accepted AI work,
reconnect, cancellation, unknown outcomes and cold recovery on one coherent
source/public/native cohort. Native/provider/Linux/model evidence is distinct.

The Notes handoff records actual image 884c3028 basic 16/16 and model 23/23,
three provider cases in one frozen-source run, complete hosted 11/11 and later
Properties 2/2 across three cold runs. Those are historical receipts, not current
server-release qualification. Preserve failed preceding attempts. CI image
execution, current published package inputs and fresh actual-host receipts must
be checked against the candidate; UI mocks cannot complete them.

## Task/editor regression

Disposition: unresolved historical source finding. The final Notes handoff records
rust-server-tasks-editors at 14/15, reproduced without the integration stack.
Its hosted application owner is now lapis-server. Reproduce the exact retained
journey against the current web acceptance and fix its owning native/API/plugin
boundary before claiming this capability qualified. Preserve dirty-editor,
ordered Task writes, stale-source and cold-recovery assertions; do not lower
assertions or infer resolution from unrelated passing tests.

## Assets and intentional opt-ins

Five native model tests remain asset-gated in the current core tests. They need
qualified ONNX/model files and their explicit model lane; a standard ignored run
is not model acceptance. Process-helper ignores exist to be invoked by parent
crash tests and must be distinguished from missing product qualification.
Actual providers remain opt-in and retain original request identities, failures,
confirmed/unknown outcomes and no automatic replay.

## Consumer and deployment boundaries

Notes owns standalone local/offline browser qualification. Desktop owns Tauri,
ordinary/e2e bundle equality and native/provider integration. Source-only
qualification never closes those owners' gaps. Deployment, image publication,
installed-app replacement and release preparation require their original
separate authority. Existing run instructions remain in the README and hosted
server runbook; no wave/status register is maintained outside this specification.

## Extracted protocol and architecture tooling

Disposition: tooling integration pending. Notes retained obsolete spec-check
commands after its crates moved here: its check:protocol alias is absent and its
source audit addresses a removed local Cargo workspace. Notes no longer runs
those native lanes. Reconcile protocol-export drift and core dependency/source
checks in this owning repository, preserving generated API/schema conformance,
transport independence and source limits. The old audit also rejects three
current feature-selected local module path declarations in indexing_closed,
tasks_closed and services/mod. Resolve that source-layout contract explicitly;
do not suppress findings or claim the old audit passed. Existing Server CI
continues to run its manifest, strict Clippy and workspace tests.
