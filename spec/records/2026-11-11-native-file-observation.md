# Hosted native file observation

SRV-HST-021 adds GET `/api/v3/clients/{client}/file-events` with admitted
vault_id/generation and an optional replay cursor. The separate resource name
preserves the existing byte route for an ordinary user file named `events`.
The user authorized this additive API. Editing-intent streaming is not part of
this change; its existing one-second recovery poll remains unchanged.

## Ownership and recovery

Core owns one notify watcher per admitted vault. Events carry relative paths,
never file bytes; physical-root and symlink admission remain authoritative.
A 256-frame/2 MiB journal bounds replay, and frames exceeding 120 KiB become an
explicit reset within the browser's 128 KiB frame limit. Cursor gaps, native
errors and receiver overflow also require reconciliation. Last-client detach
and successful vault close drop the watcher; dirty-close refusal retains it.
The OS watcher remains recursive, but app-state events are discarded before
journaling or HTTP delivery except the targeted `.lapis/app.json` leaf. This
avoids a second directory-registration lifecycle solely for kernel filtering.

HTTP revalidates owner, vault, generation, origin, physical root and session
throughout a 30-second lease. Reader admission is bounded, heartbeats do not
invoke file/database commands, and responses disable caching/proxy buffering.
The browser adapter shares one stream between root and targeted subscriptions,
waits for registration, fences retirement and resumes from its cursor after
transient disconnects/deadlines. Transient replay does not force a tree scan;
explicit uncertainty reaches the API's existing reconciliation owner.

## Qualified local boundaries

Four core watcher tests cover native sharing, replay/gaps, app-state/path/symlink
filtering and retirement. Four HTTP tests cover real delivery/replay/detach,
auth/origin/scope/cursor fences, ordinary `events` bytes and session revocation.
Seven browser transport tests cover Unicode/CRLF, malformed and bounded frames,
abort cleanup, shared subscriptions/scopes/readiness, replay/auth retirement,
retirement races and deadline reconnect without unnecessary scans.

The full Rust workspace passes 793 tests, with 7 existing opt-ins unchanged, using
serial test threads. Earlier parallel runs exposed a Search IO error and a
History binding error; both exact cases pass and the serial workspace is green.
A later wider JavaScript run hit ENOSPC; generated test executables were removed
only after Rust completed, retaining runtime binaries, libraries and receipts.
No test assertion, timeout, opt-in or budget was loosened.

Server Svelte/type/App checks pass with zero warnings, all 114 unit cases pass,
protocol regeneration/schema checks pass and all 50 Storybook cases pass.
A freshly built source API consumer passes the unchanged 600-dormant-assets
startup fixture at 69 commands/77 DB over 20 post-readiness seconds. A separate
cold default-profile/real Tasks fixture passes zero idle filesystem list/stat
and zero idle DB calls over 20 seconds, with 600 dormant assets and 200 cache files.
External create, modify, rename and delete reach the Explorer and clean editor.
These fixtures do not enable every plugin from the live personal vault.

Temporary source API qualification uses an ignored pnpm hook and separate lock;
committed dependencies retain registry semver. The first full hosted run fails one concurrent Tasks recovery case: external
canonical reloads create competing compatibility-file writers. The API owner
has a focused fix under requalification; the original assertion is retained.
Full hosted-suite requalification, released API intake, hosted CI and actual
public-image acceptance remain gates.
Source evidence is under `.local/live-vault-diagnosis/` and the API archive digest
is recorded in `.local/watch-source/provenance.json`. No npm or crate publication
occurred. The deployed image is not qualified by these source-consumer results.

## Final source-consumer qualification

The original two-App Tasks recovery failure is repaired in the API owner,
qualified Notes commit `abe9a507050ef2bf39efcbdbc83c47162910bb45`. External
configuration observation no longer asks another App to rewrite compatibility
data; local saves/removals retain their original writers. Two failing source
regressions, 108 affected cases, all 1,758 API cases, root lanes and all 275
Chromium interaction/axe cases pass. The original concurrent Tasks journey
passes unchanged, including its data.json assertion, one submit, zero retries
and clean shutdown. The source archive SHA256 is
`f506b69133215f0fc5e72fd967bad9ddf6bb85a73c7c53a78d4b6fe8827d4dea`; its runtime
files are verified equal to that committed source.

The complete rebuilt Server hosted lane passes 36 journeys, retaining the same
five opt-ins. It covers static/preview/development content controls, native
files, two-App editing, interrupted Tasks, guarded leave, cold restart and the
actual Rust static host. All 114 unit and 50 Storybook cases also pass with this
archive. Final core startup is 70 commands/75 DB, preserving <200/<100, 600 dormant
assets and the 20-second sample. A deliberately invalid inherited owner token
does not affect the isolated fixture. The enabled Tasks/default fixture retains
zero idle list/stat and zero idle database calls; total journey counts include
startup and subsequent external edits, and are not the core startup budget.

These remain source-consumer results. API 0.2.8 publication is held, normal
registry dependency intake is required before the Server feature PR, and actual
public-image acceptance remains separate. No npm or crate publication occurred.

## Independent default-image delivery

The Homebrew-free default/JJ slice passed all eight hosted checks, including
AMD64/ARM64 image tooling, and merged as
`e69c10d020f4bc70b36b272ddde83923adb9446e`. Its source AMD64 runtime measured
376,160,256 unpacked bytes with functional JJ init/clone and original lifecycle.
Exact-main CI 43124555437 and image release 43124555621 were in progress at the
once-per-turn inspection. Anonymous loader and all four agent Mod indexes are
public for e69c10d on both architectures; the full/slim Server tags returned 404
at the public probe. Published-image identity/size/runtime acceptance remains
pending. Receipts are under .local/public-mods-acceptance.
