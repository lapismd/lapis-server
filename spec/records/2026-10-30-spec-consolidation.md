# Specification consolidation — 2026-10-30

The server specification now owns the remaining shared native-core and hosted
work inherited from Notes' retired migration plans. Current chapters and
verification describe behavior; backlog topics retain feature gaps, original
acceptance boundaries and dependencies without another milestone ledger.

Notes' later cutover, image/model, provider and Properties receipts supersede its
older active-cutover prose at their recorded revisions. They do not qualify the
current server release cohort. The backlog preserves unfinished participants,
context/handoff, native tools/undo, memory, durable jobs, optional legacy History,
Docker execution and hosted/editor/model/consumer qualification.

The Notes source-check wiring was obsolete after extraction: its check:protocol
alias is absent and its core audit addresses a removed workspace. Running that
old audit here reports three current local module-path declarations; this is
recorded as owning tooling reconciliation, not accepted as a passing audit.
Existing Server CI's manifest, strict Clippy and Rust workspace gates continue.

Validation: spec-validator 0.2.0 validates nine chapters and 29 requirements with
zero findings; spec:check passes validation, mdBook and spec-first (3/3 lanes).
The Storybook static build passes with the three new metadata-only backlog
mirrors and SUMMARY-aligned navigation. Doctor and a semantic QMD search pass
under Node 24.10.0. Changed-file Prettier checks pass. No runtime/native/provider
acceptance or deployment is claimed by this documentation change.

The only requirement wording change is SRV-GOV-001's contract/acceptance review
terminology replacing milestone review. All 29 identities and product constraints
are retained. Supporting historical evidence remains outside current navigation.
