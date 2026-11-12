# Authenticated hosted vault restoration

## Contract and ownership

SRV-HST-022 adds Server-owned browser restoration to the existing authenticated
vault-opening flow. The previous initializer and successful login always selected
the chooser; neither remembered a ready workspace. Notes retains App, plugin,
layout, file-writer and recovery ownership. Desktop and plugin activation are
outside this change. No public HTTP route or npm package changes.

The browser stores only `{schemaVersion: 1, vaultId}` in origin-local storage,
under an authenticated-username key. It revalidates the ID against the current
accessible, unarchived vault list and calls the existing preparation/admission
path. Native client IDs, CSRF tokens, credentials and filesystem paths are never
stored. The backend's private `current` type exposes its existing username field.

An attempted restoration is consumed before preparation and rearmed only by the
matching live workspace's readiness callback. Failed preparation therefore
returns to the existing chooser without a repeated automatic attempt. Shared
startup failures retain their existing recovery UI. A retired host cannot admit
a later authentication result or remember a stale readiness callback. Blocked,
malformed or oversized browser storage cannot prevent normal selection.

Explicit Manage Vaults and successful logout clear the preference. Returning
intentionally to an already-ready workspace remembers it again. Expired
authentication keeps the preference behind normal login; only the authenticated
owner's entry is considered. Existing vault switching and writer-protected
teardown remain unchanged.

## Narrow regression evidence

The exact hosted regression is:

```sh
pnpm exec playwright test -c web/playwright.rust-server.config.ts \
  web/e2e/rust-vault-restoration.spec.ts \
  -g 'authenticated reload restores the selected vault and saved note with a fresh client'
```

It freshly builds the registry-backed frontend, opens a note, waits for the
existing layout write, reloads without another vault selection, and requires
the ready workspace, saved note and a different newly admitted native client.
The initial fixture attempt treated a not-yet-written layout as fatal ENOENT;
its corrected poll retains the original content assertion and deadline.

Before the product change, the regression fails at the unchanged 30-second
post-reload ready assertion, displaying the picker. Native shutdown also refuses
the lost browser's retained editing claim. The first after run passes all reload,
note and client assertions, while that deliberately retained claim still blocks
shutdown. The completed journey additionally retires only its old test-owned
client, reviews the saved file through Problems and explicitly discards its
retained claim, then leaves through the existing guarded UI. It does not add
automatic claim recovery/discard, replay native writes or relax shutdown.

The first combined run also exposed layout/editor-state contamination between
the new cases sharing one backend. Each restoration case now owns a fresh cold
Rust process/profile. No shared runtime, assertion, deadline or existing opt-in
is changed. The separate host-retirement regression first reproduces one late
admission after unmount; the host lifecycle fence repairs that boundary.

## Validation and delivery gates

The exact retirement regression passes after its before failure. All 130 unit
cases pass, including 16 new restoration/lifecycle cases. The three cold Rust
restoration cases pass, covering reload with a fresh client, intentional chooser return,
explicit workspace return, logout, and authentication expiry. Unit cases cover
owner isolation, missing/archived choices, failed admission without a remount
loop, malformed/oversized state, storage failures and retired-host fencing.
Catalog fixtures use independent preference storage; restored-workspace and
unavailable-choice stories use the production component in both themes.

All eight focused new catalog cases and the complete 58-case interaction/axe
lane pass. The fresh built catalog passes all four static checks. Type/Svelte
checking reports zero errors/warnings and the App ownership audit passes.
The latest Deno-free spec-validator is 0.2.0; doctor, QMD discovery, specification
validation, mdBook and spec-first pass. Initial acceptance-bullet and fixture
typing diagnostics were repaired at their source, without weakening checks.

The first full hosted run passes five host variants and 10 ordinary Rust cases,
then reports four stale navigation expectations in chat, two participant cases
and queue recovery. Each waits for the old picker button after the workspace
has already restored. The repaired cases await actual readiness while retaining
all conversation identity, delivery, prompt-count, cancellation, unknown-outcome
and explicit-resume assertions. The exact chat case and complete three-case
participant/queue family pass. The subsequent complete ordinary Server group
passes 14 cases with its existing signed-install opt-in skipped; all five
static/preview/development host variants also pass.

The later History restore case reproduces one old chooser-heading expectation
on preserved-authentication restart. Its repaired helper distinguishes initial
manual selection from automatic cold restoration. The exact restore case then
passes, and the complete History group passes two cases with the existing Backup
and original-source opt-ins skipped. The held original-source helper receives
the same navigation correction without enabling its quarantine.

The shared Tasks fixture initially fails its exact guarded-switch case on the
old chooser expectation for a second browser. It now observes fresh admission
before navigation and accepts authenticated chooser, opening or restored states;
its early-admission mode and all domain assertions remain intact. The exact
case passes. A subsequent full family passes 14 cases and finds one original-vault
reopen assumption: reload correctly restores the newly created other vault.
An explicit chooser return repairs that mismatch, but the next full run retains
one layout-save error caused by its immediate raw navigation. The trace records
an aborted workspace-layout PUT. The switch scenario now uses the existing
guarded chooser/open flow without unloading its live workspace. Its exact
regression passes with the strict empty-runtime-errors assertion retained.

Provider reload and held Metadata replacement steps receive the same readiness
expectation. Existing opt-in guards remain unchanged: the three provider cases
are collected and skipped, and held Metadata/original-source coverage remains
unqualified. No provider call, actual-model run or quarantine removal is claimed.

The final complete Tasks family passes all 15 cases. Across the canonical
hosted configurations, the final qualified segments total 39 passed and the
same five existing opt-ins skipped:

| Configuration                               | Passed | Existing skips |
| ------------------------------------------- | -----: | -------------: |
| Hosted Notes host variants                  |      5 |              0 |
| Ordinary Rust Server, including restoration |     14 |              1 |
| History                                     |      2 |              2 |
| Metadata and actual model                   |      0 |              2 |
| Properties                                  |      2 |              0 |
| Tasks                                       |     15 |              0 |
| Built static Rust frontend                  |      1 |              0 |

The host variants and ordinary Server group are the completed segments from
`pnpm test:hosted`; History, Metadata, Properties, Tasks and static were completed
with their matching `pnpm exec playwright test -c web/playwright.rust-<name>.config.ts`
commands after the navigation repairs. This is segmented local qualification,
not a claim that an earlier interrupted full command passed. A fresh full
`pnpm test:hosted` invocation remains in hosted CI. The five default exclusions
are signed-plugin installation, Backup screen, original-source screen, Metadata
screen and actual ONNX; they are not passing acceptance.

`pnpm test:startup-budget` passes against the checkout Rust binary and freshly
built checkout frontend: 69 commands and 75 database requests, with 600 dormant
plugin assets and the unchanged 20-second post-readiness sample. The original
strict limits remain fewer than 200 commands and fewer than 100 database
requests. This is source-consumer evidence, not public-image or all-enabled-plugin
acceptance. The final full type/Svelte check and App ownership audit pass after
all browser-helper changes. No Rust source or package dependency changed.

Receipts, original failed attempts and browser traces remain under
`.local/vault-restoration/`. Qualification uses test-owned profiles and no
production vault. The Rust implementation and published Notes package cohort
are unchanged. Exact-head hosted CI, postmerge exact-main CI and a new actual
public-image restoration run remain delivery gates; the earlier `ad7cdd1`
image predates this fix. Existing five hosted opt-ins retain their scope.
