# Admitted remotes browser transport

This slice adds remote-settings read/control and raw working-copy observation
ports to the existing RustVaultClient. Only its admitted descriptor supplies
client, vault and generation. Commands preserve native review IDs and expected
revisions. Transport fences preflight and delivery after detach and never retries
a mutation. Raw working-copy JSON retains exact native identities/timestamps.

## Before and after

Exact command: `pnpm exec vitest run --project unit
web/src/hosted/rust/repository-remotes.test.ts`.
Before implementation: zero pass, three failures. Two methods are missing; the
lost-response fixture additionally times out during its teardown after the
missing-method exception. After implementation: three pass, zero failures.
The assertions retain exact encoded route/body equality, accepted late-reply
fencing and exactly one publication request. No retry or timeout is added.

## Wider validation

- `pnpm check`: zero Svelte errors/warnings; App ownership passes.
- `pnpm test`: CLI distribution tests pass; Web unit suite is 135 pass across
  32 files, zero failures.
- `pnpm build`: production Web build passes. Existing chunk-size warnings remain.
- `pnpm spec:check`: initial authored paragraph violated acceptance-details
  structure, then the mapped governance chapter was missing. Both document
  issues are corrected without validator exceptions. Final specification check
  passes all three lanes and App ownership.

`pnpm format:check` passes for the full repository.

The new ports have no production callers in this slice. Runtime/core and HTTP
routes are unchanged. Full application settings/working-copy adoption, browser
clipboard/URL handoff, actual browser transport, automatic scheduling and both
application releases remain outstanding. Earlier native/HTTP acceptance does
not qualify those missing application boundaries.
