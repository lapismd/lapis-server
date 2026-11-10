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
static browser cases pass. Full retained hosted acceptance is recorded after its
current run completes.
Hosted CI, actual public image identity and request-budget acceptance remain
separate release gates; this test's old backend does not qualify a fresh source
server executable or prove deployment. The official plugin-install release-asset
acceptance opt-in remains governed by its existing record.
