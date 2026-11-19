# Server Terminal appearance review

Date: 2026-11-19. Requirements: SRV-GOV-004 and SRV-HST-020.

## Reviewed slice

Server owns bundled JetBrains Mono regular/bold (14px) and passes font options
through TerminalPlugin's new public constructor extension. Desktop is unchanged
and retains the user's configured/account shell. No Desktop preference, runtime
or shared lapis-terminal change is included.

Eight `Shell/Server Terminal` stories use the production hosted workspace,
Terminal registration, client adapter, panel and Ghostty renderer. Disposable
ANSI transcripts preview cyan paths, magenta eight-character UUIDs, green/red
prompt arrows, coloured directories/executables and nested directories. `pwd` in
the fixture deliberately retains the full UUID. The stories open Welcome.md and
collapse both sidebars through real workspace controls. Multiple-terminal stories exercise conditional Open, explicit New, numbered tabs, independent input and retained output.

This is a design-review milestone. Native Bash profile implementation, startup
configuration, opt-out and cold image/PTY qualification remain in the backlog.
Catalog output is not a working shell or evidence that these native gates passed.
The user requested Storybook review before PR creation; no PR, release, deployment
or baseline update occurred.

## Validation and earlier attempts

- Original `pnpm exec vitest run --project unit web/src/hosted/rust/terminal-registration.test.ts`: 2 passed, 0 failed.
- `pnpm exec vitest run --project storybook stories/ServerTerminal.stories.ts`: 12 passed, 0 failed (desktop/narrow interaction and accessibility cases); rerun after final workspace setup also 12/12 before adding multiple-terminal scenarios.
- Full `pnpm exec vitest run --project storybook`: 58 passed, 0 failed.
- `pnpm test`: 110 passed, 0 failed across 28 files.
- `pnpm check`: final 0 errors/warnings and App ownership audit passes. An initial registry-only attempt reported five type errors (two missing unpublished appearance contracts and three missing story args); the packed upstream extension and meta args resolved them. The added Welcome.md setup briefly used Playwright's `exact` option with Testing Library (one type error); removing that option resolved it.
- `pnpm build`: passes and emits bundled JetBrains Mono font assets.
- `pnpm build-storybook`: passes. Final rebuild includes the latest note/sidebar setup.
- `pnpm spec:check`: validation, mdBook, spec-first and App ownership pass.
- Changed-file Prettier check initially reported one story after a one-line edit; formatting resolved it.
- Browser review opened the actual dark Server story, verified its successful interaction state, hid the addon panel and retained the tab for the user. No screenshots were used as functional or native acceptance.

## Source and environment

The isolated Server workspace is based on the existing in-progress Terminal
integration snapshot; its original checkout is untouched. The owning plugin was
built/tested independently, packed, and installed via the ignored
`.local/terminal-preview/pnpmfile.cjs` hook with lockfile writes disabled.
Committed package manifests and lockfiles keep registry semver resolution.
The ignored provenance receipt records archive checksums and source revisions.
No source alias, node_modules edit or copied production implementation was used.

The initial review archive SHA-256 was
`22db2b1d8815581a6feffcaa2da53279402a815eb9be4d379fb755535446b12c`.
A later metadata-only pack preserves the runtime and widens peers to retain old
consumers alongside the Server cohort.

The host runs Node 26.9.0; Server declares Node 24. QMD native modules do not
match the active Node ABI, so semantic search uses the documented fallback and
`spec-validator doctor` retains that environment failure. This is local browser
review evidence, not Node-24 CI parity. The existing Tasks package also reports
a Core peer-range warning. Neither emitted a functional or accessibility failure
in the required renderer lanes.

## Final follow-up qualification

The owning Ghostty renderer now fills its CSS viewport without reserving a 15px
scrollbar gutter or stretching cells. The scrollbar and hit target meet the right
edge. Its upstream default-colour fix updates native VT cells as well as canvas
colours. The plugin owns a separate Ghostty WASM instance per renderer, preserves
richer bound session state during layout projection, and retires creations whose
renderer was already cancelled. These source fixes are committed and packed;
they are unpublished prerequisites, not registry qualification.

Final evidence (zero remaining functional/accessibility failures):

- `pnpm exec vitest run --project storybook stories/ServerTerminal.stories.ts`:
  16 passed across desktop and narrow viewports. All terminal stories start with
  the left sidebar collapsed and assert font loading, coloured canvas pixels,
  exact theme background and full-width geometry.
- `pnpm test:storybook`: 62 passed across the complete catalog.
- `pnpm test`: 110 passed across all 28 unit files.
- `pnpm check`: zero errors/warnings; App ownership passes.
- `pnpm build`: passes with bundled fonts and the qualified public package archive.
- `pnpm build-storybook` and `pnpm test:storybook:static`: build passes, four
  built-static browser checks pass.
- `pnpm dlx node@24.15.0 node_modules/@playwright/test/cli.js test -c
web/playwright.terminal-review.config.ts`: two passed from a fresh Storybook
  process. Each theme mounts five times using actual Reload story, checks drawn
  glyphs per canvas, exact background and width at device pixel ratio 2, types
  pwd through real browser keyboard events, and verifies the unabridged UUID.
- `pnpm spec:check`: passes. Spec-validator doctor passes; QMD is explicitly
  disabled in this repository, so specification search reports its rg fallback.

Recorded intermediate failures:

- Initial cold runner attempt exited 254 because its webServer cwd was wrong.
  Two later runs stalled while accumulating and asserting an unbounded glyph
  array. The final harness uses bounded per-canvas capture; no assertions were
  removed. Node 24 is used for the dedicated Playwright command.
- Multiple-terminal scenarios initially failed four cases (12 passed), exposing
  null session IDs in early snapshots. Source regression: one failed/one passed,
  then two passed after omitting unbound IDs. A temporary retained-session cache
  caused blank output in 14 cases and was discarded; it is absent from the final
  source. Fixture ownership was made explicit by the terminal fixture boundary.
- Synthetic Storybook keyboard setup lost input focus in the multi-story sequence
  (four failed). The story checks session isolation through the public Terminal
  client; the separate cold Playwright test proves actual keyboard input on every
  reload. A first public-client lookup used a nonexistent getPlugin method (four
  failed); the final story uses the published plugins Map and waits for binding.
- Intermediate typing attempts added one invalid Testing Library exact option
  and three fixture-access type errors; the final check has none.

The plugin's final source lanes pass 31 unit and six real-browser tests, check,
spec, build and publint. Upstream Ghostty passes 361 tests, format, lint,
TypeScript and the normal library/WASM build. Source evidence is separate from
these consumer results. The owned disposable WASM build container was removed.

Local provenance: plugin 0ebe84bb02615139397f37813326d878c1d28b07, archive SHA-256
`e63c3c00a572d76a09c2d16adbe9375d8fb017672540de0fc77795a487998d16`;
Ghostty 184ae982, archive SHA-256
`0cdc8a5a461bcc03384dc133acd55add1f74e66c588e363654e263e43f2b893d`.
Ignored pnpm hooks install both with lockfile writes disabled. Manifests and
lockfiles retain registry semver ranges. No Desktop source, shell preference,
PR, publication, deployment or visual baseline was changed.

## Pull request requalification on current main

The review slice is now based on Server main 3a262137, excluding the original
pre-existing integration stack. The Terminal prerequisite was ported onto its
current main while preserving activation fencing, bounded pending input and
leaf-owned session handoff (lapismd/lapis-plugin-terminal PR #5). Its local pack
is built from source 78fada88968f49c2d328785e0834aba4a1ff1fb8 and qualified with
coder/ghostty-web PR #232. Both Lapis PRs remain drafts until released packages
are adopted; the current registry lockfiles deliberately remain portable.

New-head results: 16 focused Terminal cases, 92 complete catalog cases and 228
unit cases pass. Dependency-intake and release-workflow script checks pass;
check, application build and canonical spec checks pass. The built catalog's
five browser cases pass. The cold reload/real keyboard lane passes twice, with
five mounts per theme at device pixel ratio 2.

Current production responsive layout hides the bottom panel in the mobile shell.
The original manager-inclusive 1063px harness therefore failed twice after the
port (no terminal canvas). The dedicated desktop-terminal harness now uses
1536px so the preview remains in desktop layout with Storybook's sidebar open;
all glyph, colour, geometry and real keyboard assertions are retained.

The first canonical check after port exceeded the existing governance line and
verification byte limits (two diagnostics). Terminal policy was split into its
own topic and metadata-only catalog mirror, with SUMMARY and story order updated.
The first split check identified both missing mirror and order (two diagnostics),
now fixed. Existing verification links and source-ownership requirements remain.
The first doctor/search attempt on host Node 26 reported a QMD native ABI
mismatch and used the printed rg fallback. Repeating doctor and search under
supported Node 24 passed (16 doctor checks); no global rebuild or configuration
change was needed. Canonical checks also pass.

The Terminal draft's registry CI currently reports exactly the two unreleased
Ghostty behaviours: live default-background update and full-width geometry.
Those regressions pass with the qualified upstream archive; no test is skipped
or loosened. This Server draft likewise requires the Terminal constructor
extension to be published before registry-only CI can qualify it. These are
explicit delivery prerequisites, not successful registry acceptance.

Ported Terminal archive SHA-256: `bb5249592759fdfd234518248d7041585a2369a160f77f2658a8d7e81bbd36bb`.
