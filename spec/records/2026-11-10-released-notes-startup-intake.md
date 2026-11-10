# Released Notes startup and install-queue intake

Server consumes normal registry ranges for API 0.2.7, Workspace 0.3.5, UI 0.1.8
and File Explorer 0.2.3, published by Notes release run 42892698186. The source
fixes and install-queue qualification belong to Notes PR 85; this receipt covers
Server's released-package integration. Terminal UI npm publication remains held.

The exact consumer regression is `LAPIS_DOCKER_CONTEXT=omarchy pnpm
test:startup-budget`, after `pnpm build`. It builds an isolated container using
the public c0f4aae server executable and this checkout's built frontend, a fresh
test-owned vault, 600 dormant plugin asset files, and the core File Explorer and
Markdown plugins. It signs in through a cold browser and counts native command
and database requests through readiness and another 20 seconds. It rejects 200
command calls or 100 database calls and retains JSON receipts under `.local`.
Only task-owned containers are removed; no existing application data is mounted.

Before the dependency intake, the exact regression failed with 609 file command
requests before workspace readiness (one functional regression). After intake,
three cold remote runs passed with 121/95, 121/98 and 112/95 command/database
requests respectively (zero functional regressions). The old run aborted early,
so its three database requests are not comparable to the completed samples.
The diagnostic run reached readiness at 57 commands and 58 database calls;
its final 95 database calls include 31 getMeta and 20 clearNotification requests,
not hundreds of queries per dormant plugin asset. Thresholds were not relaxed.
The fixture does not qualify all real plugins enabled together.

Frozen install, source check, 107 unit tests, frontend build, specification and
50 cold Storybook interaction/accessibility cases, static catalog build and four
static browser cases pass. The broader retained hosted run passed the login,
workspace, file/chat/participant, history and Properties families, with five
existing delivery opt-ins skipped. It found one failure among 15 Tasks cases:
the concurrent recovery assertions completed, but native shutdown refused a
retained editor claim after the second browser frame was destroyed. The exact
cold target (`pnpm exec playwright test -c web/playwright.rust-tasks.config.ts
web/e2e/rust-server-tasks-recovery.spec.ts --grep 'another App saving ordinary
settings'`) passed before editing, confirming timing-dependent teardown.

That test now awaits the second App's existing guarded Manage Vaults path before
closing its frame, just as the main fixture crosses the guarded host boundary.
Browser navigation cannot await asynchronous Svelte cleanup; this change retains
all product assertions and shutdown deadlines. The exact cold target passes
afterward; the complete 15-case Tasks family and the remaining real-server static
delivery case pass, bringing retained hosted acceptance to 35 passes with five
existing opt-ins skipped and zero unresolved functional failures. No source/runtime workaround or quarantine
was added.
Hosted CI, actual public image identity and request-budget acceptance remain
separate release gates; this test's old backend does not qualify a fresh source
server executable or prove deployment. The official plugin-install release-asset
acceptance opt-in remains governed by its existing record.

The first hosted renderer run on ca8569b failed before login with zero command
and database requests: the Username field never appeared. Its diagnostics
artifact was incorrectly excluded by the upload action's default hidden-file
filter, despite an explicit `.local` glob. Browser/network errors and bounded
page text are now emitted directly in the job log as well as failure.json; the
JSON-only artifact upload explicitly includes hidden paths. Request limits,
assertions and readiness deadlines are unchanged. The browser-startup failure
remains a merge gate until the rerun yields its underlying cause or passes.

The unchanged cold request-budget target passes again on remote `omarchy`
(112 commands and 97 database calls). A temporary test-owned fixture with a
controlled JavaScript exception before login exits unsuccessfully as expected;
its exception and page text are verified in both failure.json and the job log.
That probe is not shipped, and the hosted Linux startup remains unqualified.
