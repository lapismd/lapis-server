# Hosted queue reload quarantine

The user previously directed that demonstrated intermittent failures be documented
and quarantined rather than merging red checks. This one-case quarantine applies
that standing instruction; it is independent of the accepted audit exceptions.

## Exact boundary and evidence

`pnpm exec playwright test -c web/playwright.rust-server.config.ts rust-server-queue.spec.ts`
runs `native queue controls preserve cancellation and require explicit cold resume
in the retained chat screen`.

On eccd79d, CI job 133706025060 fails this one case after browser reload at the
queued-message-region assertion (line 184); the server group passes 14 and skips
one pre-existing plugin case. The same unchanged scenario passed in the preceding
complete hosted run and passes cold in isolation (1/1, 8.9 seconds) after this CI
failure. That is intermittent evidence, not proof of a repair. The audit-only
exception commit changes no runtime code or dependency resolution.

The native host starts normally. The visible queued region is missing after
reload; the available log does not establish whether Chat presentation, restored
selection or queue observation is responsible. Do not infer lost durable queue
entries or automatic replay from this timeout. Earlier cancellation and pause
checks completed; later cold-service-restart and explicit-resume assertions were
not reached by the failing run.

## Scope, ownership and restoration

Normal hosted runs skip only this exact browser scenario. Preserve its complete
assertions, fixture, diagnostics and timeouts. Run it explicitly with:

```sh
LAPIS_SERVER_QUEUE_RECOVERY_TEST=1 pnpm exec playwright test -c web/playwright.rust-server.config.ts rust-server-queue.spec.ts
```

Server owns reproducing hosted reload/session/queue observation. A confirmed
renderer or public plugin defect must be fixed by the owning Notes/AI package
and consumed through its released contract; no consumer workaround is authorized
by this quarantine. Native queue and other hosted Chat/participant cases remain
enabled and have separate coverage. They do not replace this browser proof.

Review by 15 December 2026. Re-enable after a deterministic cause is repaired in
its owner, the unchanged exact test passes cold, and the complete hosted group
passes with this case enabled. Retain failed traces and source/dependency inputs;
do not increase timeouts or weaken no-replay/explicit-resume assertions.

## Selection verification

With `LAPIS_SERVER_QUEUE_RECOVERY_TEST=1`, the unchanged diagnostic passes cold
(1/1, 8.9 seconds). With the normal environment, the complete directly affected
`web/playwright.rust-server.config.ts` group passes 14 and skips two: this exact
queue case and the one existing official-plugin install opt-in. No other case
is removed. Prettier, canonical spec/ownership/release-workflow checks and audit
pass with only the two previously approved CVE exceptions configured. Doctor
passes 15 checks with its existing script-alias warning; QMD discovery succeeds.
