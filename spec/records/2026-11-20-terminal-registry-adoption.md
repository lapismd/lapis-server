# Terminal 0.1.1 registry adoption

Date: 2026-11-20. Requirements: SRV-GOV-001/004 and SRV-HST-020.

Terminal PR #5 and metadata release PR #6 are merged. Public release
[`v0.1.1`](https://github.com/lapismd/lapis-plugin-terminal/releases/tag/v0.1.1)
binds source `6bcac68f330ddc9dd775082ff5209ace54fa21be`.
The initial tagged publication failed npm `ENEEDAUTH`. After the user approved
the scoped GitHub trusted publisher and completed security-key verification,
[run 44638282729](https://github.com/lapismd/lapis-plugin-terminal/actions/runs/44638282729)
passed publication, public installation and signature/attestation verification.
No bootstrap token was added. Registry integrity is
`sha512-B7J2flpC+LJtWiSr959f5LfFOV7yTpQbbPMsFXcWUj6NKcZCG4u4AlSHfMBpv6E+lP64oVcHOBRbaH6Zimiyqg==`.

Server now declares `^0.1.1` and locks registry Terminal 0.1.1. Its former
Ghostty runtime dependency leaves the lockfile because Terminal bundles the
version-pinned patched renderer and matching WASM privately. The ignored
Terminal archive hook was retired. All acceptance below uses an ordinary
`pnpm install --frozen-lockfile --ignore-pnpmfile`; no source archive, consumer
patch, source alias or node_modules modification is needed.

## Registry consumer qualification

- Original `pnpm check`: two missing-contract TypeScript errors with locked
  Terminal 0.1.0, then zero errors/warnings with registry 0.1.1. App ownership
  and all five release-workflow checks pass.
- `pnpm exec vitest run --project storybook stories/ServerTerminal.stories.ts`:
  16 passed, zero failed across desktop and narrow cases.
- `pnpm test`: 123 dependency-intake checks, nine CLI/release tooling checks
  and 227 unit cases passed, zero failed.
- `pnpm test:storybook`: all 92 catalog interaction/accessibility cases passed.
- `pnpm build` and `pnpm build-storybook`: passed with the public package.
- `pnpm test:storybook:static`: five built-static browser cases passed.
- `pnpm dlx node@24.15.0 node_modules/@playwright/test/cli.js test -c
web/playwright.terminal-review.config.ts`: two passed from a fresh Storybook
  process, with five mounts per theme, DPR 2, actual Reload story, glyph/theme/
  width assertions and real keyboard `pwd` preserving the full UUID.
- The user-facing development Storybook on port 6088 was restarted against
  the registry install and serves its catalog successfully.

The dependency adoption is a separate four-file commit: manifest, lockfile and
the two generated canonical intake slots. Its fail-closed intake check passes.
The aggregate spec check initially reported one missing-governance mapping;
the explicit spec-first diagnostic reproduced it once. Qualification evidence
and the mapped governance reference belong to this following documentation
commit, preserving the intake commit's required scope without changing policy.
The final `pnpm spec:check` and changed-file formatter qualify this documentation.

The local host uses Node 26; the dedicated cold browser command uses Node 24.
Node-24 QMD probes reported the installed global SQLite module's ABI mismatch.
The existing Node-26 QMD runtime opens the index, passes all 16 doctor checks
(one existing script-alias warning) and completes semantic specification search.
No global module rebuild or configuration change was made. Hosted CI retains
its supported Node-24 production and browser qualification.

Upstream Ghostty PR #232 remains the patch-removal path and requires maintainer
review. Native Server Bash profile/listing output, actual image/PTY acceptance
for that profile, Desktop defaults and visual baselines remain separate.
