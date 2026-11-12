# Released API intake for native file observation

## Public package identity

`@lapis-notes/api@0.2.8` is published from Notes source
`6fbe662e67a41c7710573f5be48d44007548ad4d` through guarded
[release run 43177870187](https://github.com/lapismd/lapis-notes/actions/runs/43177870187).
Its exact-main validation run 43129629800 passed before publication. The
existing hosted Storybook interaction quarantine remains skipped; the source
qualification separately passed all 275 local Chromium interaction/axe cases.

The user approved publication of existing packages. This run selected only
API 0.2.8 and did not republish History or another package. npm initially
reported that the accepted package was still processing; the fresh registry
consumer exhausted its original retry window. After actual public verification,
only that failed verification job was rerun. Publication was not replayed.

Public npm tarball SHA-256:
`c6990f7b0393793eb90c006effa6ec67ebe658cb00cd1d965d0fd7c39eb05bb8`

Public npm integrity:
`sha512-FryXNiqul52w+xnZtkcgbXE6kO7FadTsnredD+/9IxDMEVm+3dOKr/tQY70m4nqcv4/80/hv8AG8cGx8Q5Xmpw==`

All 548 packaged file contents match the exact-main qualified artifact.
Sigstore verification checks the GitHub Actions issuer, Notes release-workflow
identity, tarball subject digest, exact repository source and authorized
publication invocation. The public
[api@0.2.8 release](https://github.com/lapismd/lapis-notes/releases/tag/api%400.2.8)
records the same integrity/source, and its tag points to the source above.
Local and hosted gzip bytes can differ despite identical file contents, modes
and modification times; public integrity is checked against the actual tarball.

## Registry consumer boundary

Server consumes normal npm range `^0.2.8`, with its registry integrity in the
lockfile and a passing frozen install. No local archive, pnpm hook, package
patch or file override enters the committed dependency graph. Workspace 0.3.5,
UI 0.1.8, File Explorer 0.2.3 and History 0.2.4 remain unchanged; other package
metadata is preserved. Updated peer snapshots only bind the released API.

The before run freshly rebuilt the actual Server consumer with registry API
0.2.7. Its exact `rust-file-observation.spec.ts` regression failed once at the
cache-path exclusion assertion: it recorded 605 calls into the fixture under
`.lapis/plugin-registry-cache/fixture`. The original 20-second startup and
20-second idle samples, cache exclusion, external-file actions and shutdown
assertions remain unchanged for the after run.

The unchanged after regression passes with registry API 0.2.8: cache fixture
requests are absent, and the 20-second idle sample has zero filesystem list/stat
and zero database calls. External create, modify, rename and delete still
update the real Explorer/editor. Its total journey counts are 165 commands and
170 database calls across startup and later external mutations; these are
distinct from the core startup-budget fixture. Two bounded stream requests
serve the journey without recursive HTTP polling.

The fresh registry build, type/Svelte check with zero warnings, App ownership
audit, all 114 unit cases and all 50 Storybook interaction/axe cases pass. A
fresh Storybook build also passes all four static catalog checks, including
the populated workspace geometry and isolated Docs shell.

The complete hosted lane passes all 36 journeys, retaining the same five
existing opt-ins. It covers the original two-App settings/Tasks recovery case,
all 15 Tasks cases, dirty editing and guarded leave, external native file
updates, restart and the real Rust static host. No assertion, deadline or
quarantine changed. The unchanged Rust source retains its earlier 793-case
qualification with seven existing opt-ins; a new Rust source change was not
part of this dependency intake.

The independent core startup fixture passes with **70 commands / 75 database
requests**, including **49 / 68 at readiness**. The original strict limits
remain fewer than 200 commands / 100 database requests, with 600 dormant plugin
assets and a 20-second post-readiness sample. This uses the current compiled
checkout backend and rebuilt registry frontend, not a public Server image.

Hosted CI and acceptance of a new public image remain required before calling
native file observation delivered. The already qualified `e69c10d` image
predates that feature. The existing one-second editing-intent poll is unchanged;
zero idle filesystem/database calls does not mean zero browser network activity.

Receipts are under `.local/public-mods-acceptance/api-028-release/` and
`.local/live-vault-diagnosis/registry-02*`, including the original failure
trace, public provenance statement and frozen-install/build logs. These are
test-owned fixtures, not the production personal vault or all plugins enabled.
