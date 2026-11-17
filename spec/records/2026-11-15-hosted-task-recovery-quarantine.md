# Hosted browser network failure: provisional quarantine withdrawn

Current status: no new quarantine is delivered. The unchanged Task recovery
case passes in isolation and in the complete stable-network run. The proposal
below is retained only as failed-attempt history.

## Observed boundary and ownership

The exact existing case is `web/e2e/rust-server-tasks-recovery.spec.ts`:
`another App saving ordinary settings preserves interrupted native Task references`.
During `pnpm test:hosted`, the Tasks lane fails 1 case and passes 14. Its data,
retained-reference, single-submit and no-retry assertions succeed. Teardown
then reports `Error loading configuration: .lapis/app.json TypeError: Failed to
fetch` from an asynchronous configuration reload as the document/server leaves.
The identical cold isolated case passes unchanged (1/1, 20.7 seconds). This
non-reproduction does not establish a product repair.

Server owns the hosted HTTP client/fixture sequencing; Notes API owns the
configuration reload and observation lifetime. A generic disposal fix belongs
at its owning boundary once the race has a deterministic reproduction. No
assertion, diagnostic, timeout or product behavior is relaxed here.

## Explicit user direction and scope

The user requested documenting and disabling an intermittent test when proven
flaky, and clarified that accepted failures must be documented/quarantined and
red checks must never be merged. Apply that direction to this one existing
case only. Normal execution marks it skipped; all other hosted Tasks cases
and every runtime-error assertion remain enabled. Skipped coverage is not a
passing recovery qualification.

Run the original case, with its original assertions and timeout, using:

```sh
LAPIS_SERVER_RUN_QUARANTINED_TASK_RECOVERY=1 pnpm exec playwright test -c web/playwright.rust-tasks.config.ts rust-server-tasks-recovery.spec.ts
```

The diagnostic lane must remain fatal on failure. Restore normal execution
after a deterministic reproduction and owner fix pass both the cold exact case
and complete hosted Tasks suite, including teardown with zero runtime errors.
Native Core reference/preservation cases and the other hosted recovery cases
retain their separate evidence; they do not replace this browser scenario.

## Subsequent diagnosis: provisional quarantine withdrawn

The later Sync-consent failure reproduces as a blank page before login. Its
retained Playwright trace contains repeated `net::ERR_NETWORK_CHANGED` errors
while loading the split JavaScript imports. This coincides with local Docker
validation workers being started alongside native-host Chromium acceptance.
It is an environment failure, not evidence that consent or Task data is wrong.
The Task failure is a failed configuration fetch under the same mixed test
scheduling; its data assertions had already passed.

Do not hide these errors or weaken assertions. Finish local Docker workers
before running native-host browser lanes. The proposed single-case skip is
withdrawn before landing, and the original Task case is re-enabled for exact
and complete stable-network acceptance. The earlier assessment above is retained
as failed-attempt history, not current quarantine authority. No permanent
quarantine or opt-in flag is included in the delivered test selection.
