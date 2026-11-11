# Core 0.12.4 dependency intake and development restart

## Failing boundary

Dependency PR 42 adopts published Design Core 0.12.4 and refreshes the five
existing Notes compatibility overrides. Notes API 0.2.7, UI 0.1.8, Workspace
0.3.5, File Explorer 0.2.3 and History 0.2.4 remain released registry inputs.
The upstream patch repairs read-only Kanban activation after data refresh;
no owning Design Core defect was reproduced in this consumer investigation.

CI run 43008185005, Notes UI job 130480850530, fails exactly
`content plugin controls the real hosted HTTP and MCP endpoints (development)`
at `page.goto(origin)` immediately after `runtime.restart()`:
`net::ERR_ABORTED`. The hosted-content/Notes group reports **1 failed / 4 passed**.
Type, unit, build, startup-budget, Storybook, static catalog and other PR jobs
passed before this failure. Logs: `/tmp/lapis-server-dependency-pr42-failure.log`.

Exact local boundary:
`pnpm exec playwright test -c web/playwright.hosted-notes.config.ts -g 'content plugin controls the real hosted HTTP and MCP endpoints \(development\)'`.
The original local cold attempt passes 1/1; the CI timing failure was not
reproduced locally. The after attempt also passes 1/1. These results do not
establish a deterministic local before-failure or waive exact hosted CI.

## Fixture ownership and repair

The prior document stays open while the fixture stops/restarts Vite. Vite's
installed client handles `vite:ws:disconnect` by polling for the restarted
server and then calling `location.reload()`. That surviving navigation owner
can race the fixture's explicit cold `page.goto(origin)`, consistent with the
hosted aborted-navigation failure.

After the existing guarded sign-out completes, the fixture now navigates to
`about:blank` before each service restart. This disposes the old development
document and HMR client before the new server is admitted. Authentication,
HTTP/MCP byte equality, foreign-vault refusal, credential retirement,
disable/re-enable and persisted selection/restart assertions are unchanged.
No product workaround, navigation retry, sleep, timeout extension, assertion
loosening or new opt-in is introduced. Shared runtime/session helpers are not
modified; all three content renderers consume the repaired fixture.

The PR also brings its prior base up to current main through a merge without
rewriting the bot's immutable commit. Historical public 7563116 receipts still
qualify their original cohort, not this Design Core 0.12.4 intake or a new image.

## Validation and hosted gate

The exact repaired development target passes 1/1. Root Svelte/type/App-ownership
checks, all 107 unit cases in 28 files (one worker), the built registry frontend,
50 Storybook interaction/axe cases, the static Storybook build, four static
catalog cases and all five hosted UI/content journeys pass. The unchanged
remote Docker omarchy source-consumer budget passes 112 commands / 75 database
requests, 600 dormant assets and the original 20-second sample; it uses the
fixture's old backend plus the rebuilt frontend and is not public-image evidence.
The complete retained native families pass **35 journeys**, with the same
**five existing opt-ins** skipped: hosted 5, server 10, History 2, Properties 2,
Tasks 15 and static delivery 1. Native browser checks use the existing local
Rust debug fixture; exact-head CI rebuilds the locked Rust source before merge. All originally enabled
functional and accessibility cases must pass; the five existing hosted opt-ins
retain their original status. Exact-head hosted CI is required before merge.
No npm, crate or registry publication is part of this repair; existing main
image publication remains authorized through the normal merge workflow.
