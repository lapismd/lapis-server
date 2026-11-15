# Server skill profile intake qualification

The exact baseline `pnpm check` fails with four type errors and zero warnings
in ServerVaultHost and the hosted backend fixtures. The older App Profile 0.1.0
consumes API 0.2.8, while ContentAccessPlugin requires the skill-management App
contract. The fix adopts the public Profile 0.2.1 / API 0.6.5 boundary and its
compatible public package cohort; no type shim, renderer copy or source alias
is introduced. The obsolete API/UI design-core peer allowances are removed.

During registry publication, `.local/profile-cohort-20261115/archives.json`
references the two immutable tarballs from the green Notes release candidate
at 488d9cf561369bc9458e1f7aae7dddd51a064c9f. SHA-256 and package identities are
checked by the existing `pnpm validation:profile --manifest` workflow. Its
ignored consumer lockfile is separate from the authoritative registry lockfile.
These archives qualify public built exports; they do not prove publication.

Rebasing the skills stack onto Server main a60f985b retains both Sync and skill
navigation. The exact packaged-skills target passes 9/9; the complete native
Core/Protocol/CLI suite passes 692 cases with seven unchanged native-model
opt-in cases ignored. Strict all-target Clippy passes with -D warnings.

Final registry resolution, hosted UI acceptance, same-vault CLI/cold restart,
image qualification and deployment remain pending.

## Registry and security baseline

API 0.6.5 and Profile 0.2.1 are published. The normal registry install passes
the original `pnpm check` target with zero errors/warnings; no archive hook is
used for this result. The temporary archive graph also passed 139 unit cases
and the production renderer build; registry qualification remains distinct.

The first registry audit reports nine advisories: six moderate and three low.
Eight have official patched releases (Hono, its Node adapter and KaTeX).
Compatible Hono updates refresh the lock; the narrow KaTeX dependency selector
uses the official patched 0.18.10 release, matching the qualified Notes runtime.
No dependency source patch, alias or advisory suppression substitutes for those
patches. Renderer/math and hosted acceptance remain required.

Only deepmerge GHSA-gcqx-w48m-w2h4 / CVE-2026-93753 remains eligible for an
exception. The user explicitly accepted this pre-existing low-severity,
build-tool-only advisory with review by 15 December 2026; upstream has no patched
release. The package audit configuration names only that CVE. Remove the ignore
when a patched version is available and revisit it at the review date.

## Current source results

After current-main rebase, the complete Rust workspace passes 874 tests with
seven unchanged native-model opt-in cases ignored. Workspace all-target Clippy
passes with -D warnings. The audit repair leaves zero actionable advisories;
only the explicitly approved deepmerge advisory remains ignored.

The new exact same-vault boundary is `pnpm exec playwright test -c
web/playwright.hosted-notes.config.ts hosted-skills.spec.ts`. Its first cold
run fails 1/1 at the missing Markdown RICH_SYNTAX.md reference while Server still
consumes Markdown 0.3.1. The real app command successfully installs the host CLI
skill and its three references before that assertion. Markdown 0.3.2 registry
intake must repair this regression before any merge; it is not quarantined or
waived. Remaining assertions cover explicit CLI write permissions, preservation
through both app and CLI updates, cold receipts and credential expiry.

The first complete static catalog lane passes four and fails one narrow
workspace case. Its exact cold reproduction also fails 1/1. Trace inspection
shows the story play stops at its desktop-only `Server DB` assertion while the
mobile session is still inert; it never reaches note opening. The story helper
must await `data-notes-session-ready`, assert the rendered mobile stage instead
of a desktop-only status bar, and open the actual accessible note button. The
editor and viewport assertions remain unchanged. This is a catalog fixture
boundary; no application layout CSS or public package is overridden.

## Final registry qualification, 16 November

Registry API 0.6.5, Profile 0.2.1 and Markdown 0.3.2 pass source checks
(zero errors/warnings), 139 unit tests, four CLI distribution tests, production
build, protocol drift checks and audit with only the scoped approved deepmerge
exception. The complete catalog passes 64 interaction/accessibility cases and
five static cases. Normal hosted acceptance passes 41 cases with five existing
opt-in skips; the previously failing Task recovery case remains enabled.
The exact stable-network consent and Task recovery cases pass 1/1 each.

The same-vault skill case passes on two cold runs (10.1 and 10.4 seconds): actual
app installation, CLI and app protected updates, modified reference preservation,
explicit read/write credential boundaries and cold credential expiry. Markdown's
five references and the CLI's three references are installed through public
package/native contracts. No renderer implementation is used for native writes.

The final cold Tasks HTTP/MCP/CLI test initially fails 1/1 because its owner
catalog assertion still expects 17 total operations. The skills provider adds
three operations. The repaired assertion retains all 17 Tasks operations,
requires exactly install/list/update in the skills namespace and 20 total owner
operations; the Tasks-only credential still sees exactly 17 operations. The exact
command `CARGO_TARGET_DIR=.local/rust-core-target pnpm test:plugin-operations`
then passes 1/1, including cold restart and credential isolation. This is an
assertion scope correction, not a relaxed permission boundary.

Mixed Docker/native browser attempts are retained in the environment record.
No provisional Task skip is shipped. Image CI, merge and deployment remain
separate delivery gates; Desktop must consume the resulting immutable revision.

The original plugin-operation acceptance fails 1/1 because its owner catalog
count predates the three skill operations (20 actual, 17 expected). The repair
asserts all 17 Tasks operations plus exactly install/list/update in the skills
namespace, and keeps the existing Tasks-only credential count at 17. The exact
`pnpm test:plugin-operations` target now passes 1/1 with real HTTP/MCP/CLI and
cold restart; no credential authority is broadened.

The unchanged Task recovery and consent cases pass cold in 19.4 and 5.3 seconds.
The full stable-network hosted lane passes 41 enabled cases with five existing
opt-in cases skipped. The new same-vault app/CLI skill case passes cold in 10.4
seconds. Docker workers were stopped before host Chromium acceptance. No new
Task skip ships. Upgrading the Deno-free spec-validator to 0.2.1 passes doctor
(14 checks, one existing script-alias warning), QMD search, canonical spec checks,
source checks and audit. Hosted deployment and image qualification remain
separate gates after this source commit.
