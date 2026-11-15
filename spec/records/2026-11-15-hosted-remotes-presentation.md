# Hosted Repository remotes presentation

This slice adopts the shared History public native-remotes service in the hosted
Repository factory. The same admitted client supplies working-copy status,
remotes and optional passive native synchronization. Core owns credentials,
network admission, merge/apply, scheduling and publication. Browser code owns
clipboard/browser handoff and rejects late delivery after client or Repository
closure. Accepted commands are never replayed after a lost response.

## Source regression and composition

The new exact command is
`pnpm exec vitest run --project unit web/src/hosted/rust/repository.test.ts -t 'exposes native synchronization'`.
It fails once with a missing synchronization service, then passes after forwarding
the native port through the production factory and Repository lifetime. It
checks exact retained apply/revision identity, no retry after a lost reply,
joining a pending passive read and refusing both late delivery and later work
after closure. The lower transport family records its own one missing-method
failure and four final cases in `repository-remotes.test.ts`.

The actual Server host, App Profile, Repository plugin, shared settings and
footer run in full-shell stories with declared memory native data. Both targets
retain independent fetch counts; the working-copy split footer has native diff
statistics and targeted actions. Recovery stories exercise explicit exact apply
through the same production adapter, in light/dark and wide/narrow layouts.
Fixtures do not perform native transfers or qualify real credential custody.

The prior 390px failure comes from a Server helper assuming the desktop footer
is visible in mobile layout. The helper now opens the actual mobile sidebar and
waits for session-owned readiness. Independent Profile startup fixes are
qualified and released from Notes separately; no Server wrapper hides them.

## Qualification inputs and results

The final tracked cohort uses normal registry ranges: API `^0.6.5`, UI
`^0.2.1`, Workspace `^0.4.6`, App Profile `^0.2.2` and History `^0.5.1`.
Frozen registry installation succeeds with no source alias, pnpm hook or archive
overlay. Earlier private Profile/History archives were qualification inputs only
and are retired; their initial checks passed 161 units and 76 catalog cases.

Final `pnpm check` reports zero errors/warnings, including App ownership. All
162 units across 34 files pass. The cold complete catalog passes 76 cases across
six viewport-expanded files. Renderer and static Storybook builds pass; all five
static browser acceptance cases pass. Doctor succeeds with the existing optional
script-alias warning, and semantic QMD search runs on released Node validator
0.2.0. Stories remain visual-pending; no baseline is changed.

## Cold compiled host browser acceptance

The actual released Server source is `c32b5ae48b24c98dfa8de2004cf68e0496cd63bd`,
qualified/merged in PR #70. GitHub image run `43774428216` succeeds. The image
manifest digest is `sha256:8280491f4ffbc49b120417552d559bf13bbfe409f2e861e5697837e1063242d5`;
its OCI revision matches that source. A disposable local launcher executes its
real `/app/lapis-server`, with the final built renderer mounted read-only, fresh
private storage outside repository ancestors and a separate random 32-byte key.
No existing application profile, OS trust or user credential is reused.

Exact command: `LAPIS_TEST_RUST_STATIC_ROOT=<final web/dist> pnpm exec playwright test -c web/playwright.rust-history.config.ts rust-server-remotes.spec.ts`.
The first case fails because a plain-folder fixture expects an Original repository
section. Giving the fixture an actual Git-backed JJ repository fixes that
assumption. The second passes save and custody but times out expecting a vault
picker after cold restart; the host correctly restores its saved workspace.
The repaired journey accepts the real startup surfaces and passes 1/1 without
changing production code or deadlines. It proves both target settings, saving
without network publication, manual synchronization default, actual encrypted
credential save/forget, process restart persistence, browser detach, guarded vault
closure and absence of token bytes in logs/private files. Requests are observed
through the actual authenticated routes, including their query-scoped admission.

Cold compiled HTTPS/SSH transport, rejection, uncertain-outcome recovery and
locked-key custody are qualified independently in the backend receipts merged
with PR #70. This browser test performs no remote transfers and cannot replace
those receipts. Desktop and live forge sign-in remain independent gates.

## Remaining release gates

Complete final formatting/specification and exact-head hosted CI, merge this
presentation slice, then qualify its GitHub image/release. Standing user approval
covers release and workflow gates. No application-data migration is required.

## Editing-intent admission

The production History-only factory initially omits native editing intents
because they are gated only by Tasks selection. The exact new case
`pnpm exec vitest run --project unit web/src/hosted/rust/native-editing-intents.test.ts -t "supplies native editing guards"`
fails once with the missing port. History selection now supplies the existing
admitted editing bridge independently of Tasks. This preserves the native
scheduler's dirty-editor guard without adding another writer or IPC authority.
The repaired exact case, complete editing family and wider results are recorded
before adoption commits; actual cold host acceptance remains required.

The complete cold hosted-history lane passes three cases with two existing
opt-in skips (backup/original recovery screens); the new remotes journey runs
again in that lane. The first final spec check refuses the governance chapter
at 409 lines; moving its complete remote qualification paragraphs to the owning
remotes chapter preserves the contract and restores its size bound.

## Concurrent-main release authentication intake

Rebase onto `a63b96a2` retains main PR #92's repository App authentication,
version-PR regression script, YAML dependency and concurrent Lapis Sync evidence.
Three conflicts combine the new YAML manifest/lock entry with the final registry
cohort and preserve both governance topics. There is no browser or Rust behavior
change. Frozen installation, formatting, the three specification lanes, source
ownership, zero-diagnostic check, both release-auth regressions and all 162 units
pass again. Earlier native image/browser receipts keep their original source;
main's intervening change does not alter native implementation or the renderer.
