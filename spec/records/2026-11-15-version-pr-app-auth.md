# Version PR App authentication

Revision: enclosing Jujutsu change.

## Contract and acceptance review

SRV-GOV-001 now requires canonical-main, repository-scoped dependency App
authentication for generated version PRs and non-persistent checkout. Missing
credentials fail closed. All native, image, renderer and publication gates remain
unchanged. The existing release helper keeps the workflow credential through
`GH_TOKEN`, preserving GitHub tag/release creation and image/CLI dispatch;
Changesets itself does not create GitHub releases. No runtime, public package or
container source changes, and no release is performed by this slice.

## Verification

- `node --test scripts/version-pr-auth.test.mjs`: two failures before the
  workflow change; two passes afterward.
- `pnpm spec:check`: canonical validation, mdBook, mapped spec-first,
  App-ownership audit and both auth regressions passed.
- `pnpm format:check` and pinned actionlint on the changed release workflow:
  passed.
- `pnpm check`: zero errors/warnings, App-ownership audit and both auth
  regressions passed.
- `pnpm test`: six Node release-tooling cases and 138 unit tests passed.
- `pnpm exec spec-validator doctor --json` and `pnpm spec:search 'release Changesets'`:
  doctor is healthy; QMD returned the owning governance chapter.

Parsed workflow coverage checks the App scope, absence of fallback, API commits,
action pins, canonical main and separation of PR versus publication credentials.
The regression runs in normal check, test and specification lanes.

## Remaining qualification

Hosted source CI and the next eligible App-created version PR remain separate
acceptance. No artificial Changeset or release bump is introduced. Native Rust,
container and browser lanes are unchanged and are not claimed from these tooling
and unit checks. No repository protections or visual baselines changed.

## Current-main rebase review

Rebased onto `7efbdca64b47635bb07814f80d82a89490a99ca1`. The incoming guarded
hosted-fixture departure requirement and the App authentication paragraph are
both retained. Exact auth/publisher-token regression passes 2/2; specification,
App-ownership, formatting and pinned release-workflow lint pass after resolution.
No publication, image/CLI dispatch or repository protection setting changes.

## Subsequent current-main rebase

Rebased onto `c32b5ae48b24c98dfa8de2004cf68e0496cd63bd`, retaining all
incoming native account/remotes work, pinned manifests and existing evidence.
The exact auth/publisher-token regression passes 2/2; `pnpm check` reports zero
errors/warnings, and specification, App-ownership, formatting and pinned release
workflow lint pass.

The rebase first exposed the 400-line governance and 32768-byte verification
limits: main's governance chapter already had 403 lines, and an expanded padded
evidence row enlarged verification beyond its byte limit. Four concurrent Sync
qualification paragraphs moved verbatim to the existing linked governance topic,
and the verification summary retains every incoming evidence link plus the App
authentication record. The chapters now fit at 399 lines and 32745 bytes,
respectively; no limit, native assertion, publication gate or credential changes.
