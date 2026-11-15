# Lapis Sync opaque operation journal — 2026-11-15

This second clean Server Core slice extends the disabled object repository at
`cbf19800`. It does not expose a host route, select Sync for an application or
claim that opaque bytes have passed Plugins-owned signature, membership or
content-key validation.

## Exact unavailable boundary

The new target initially failed before collection because
`OperationJournal`, `OperationKind` and `OperationPhase` did not exist:

```sh
cargo +1.99.0 test -p lapis-app-core \
  --test sync_operation_journal --locked -- --nocapture
```

After the first implementation, three cases passed and two failed because
durable retry/corruption errors were misclassified as invalid input. The final
target passes **5/5** and strict affected Clippy passes.

## Implemented boundary

One `OperationJournal` is bound to the admitted physical profile, logical vault
UUID and explicit destination. A private owner lock prevents two native writers
for the same destination; replacement of the profile, journal root or owner
identity fences the retained instance. Journal storage remains outside managed
vault content.

Each record contains only opaque sealed bytes plus bounded native authority and
state metadata. The record pins its ciphertext SHA-256 and stable request/UUID
identity. An exact retry reuses the durable record at its current phase; changed
ciphertext, request identity or operation kind cannot reset it. Conditional
publication preserves the previous bytes on stale transitions or corruption.

The admitted transitions are `staged -> accepted`, `staged -> unknown`,
`accepted -> applied`, `accepted -> unknown` and `unknown -> accepted` after
explicit inspection. Unknown work cannot jump directly to applied, and applied
work is terminal. Cold reopen retains the exact sealed intent and final phase.

The exact cases cover retry reuse after acceptance, unknown-outcome inspection,
cold reopen, stale phase/request/kind refusal, independent destinations,
exclusive ownership, replaced roots, vault-content overlap, invalid destination,
corrupt records and sealed-payload bounds. Every production file and exact test
remains below the repository line limits.

## Qualification

- Exact journal target: **5 passed, 0 failed**. Combined object/journal targets:
  **12 passed, 0 failed**.
- Strict affected and complete-workspace Clippy: passed with warnings denied.
- Complete `lapis-app-core`: **585 passed, 0 failed, 7 ignored**. The ignored
  cases are the repository's governed native-model asset lanes; no Sync case is
  ignored.
- Complete Rust workspace: **812 passed, 0 failed, 7 ignored**. The retained
  linker compact-unwind warning does not fail the workspace.
- Web unit lane: **132 passed, 0 failed**. `pnpm check` reported zero Svelte
  errors or warnings and passed the App-ownership audit. The production web
  build and `pnpm spec:check` passed.
- Spec search resolved the changed canonical chapters and `spec-validator
doctor --json` was OK apart from its existing repository-specific script
  alias warning.

The JavaScript lanes ran under local Node 26.9.0 and emitted the manifest's
unsupported-engine warning for required Node `^24`; they are source evidence,
not the required Node 24 CI qualification.

## Unclaimed boundaries

This is durable native state, not content encryption or protocol admission.
Plugins-owned code must still validate signed events, attestations, membership,
dependencies and key epochs before an operation advances. Desktop/Server
custody must supply the actual encrypted payload. Events, HEAD discovery,
working-file apply, HTTP, Docker/image, Tauri, OS custody, independent devices
and installed applications remain unqualified.
