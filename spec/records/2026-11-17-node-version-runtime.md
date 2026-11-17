# Server Node version runtime guard

This source slice preserves SRV-GOV-001/003 and the existing private
Server release boundary. The earlier [dependency preparation](2026-11-16-version-runtime-preparation.md)
retains its original failures and functional evidence. The separately approved
[KaTeX decision](2026-11-16-katex-audit-exception.md) remains unchanged; this
slice grants no advisory exception and does not claim a clean production audit.

## Whole-recipe hosted proof

Shared CI PR26 merges as `121359a245a334da31bc138933595a5f00f5ba8b`.
Read-only run `44016373469` succeeds against owner source
`78a3fe6f84b008af6ef11242c352a33b146e2caa`. Its artifact `14299356248`,
`server-changesets3-runtime-qualification`, is downloaded and its ZIP SHA256
verified against GitHub's advertised digest:

```text
7ab2487a4d71dd63031d8b3a8e1e104f66f769a2a7781f5470a2788633906a73
```

The receipt binds the exact source, lock, Changesets3.0.3 bytes/config,
pnpm10.34.5 and formatter inputs. It executes the unchanged
`pnpm release:version` on Node24.15.0 with Deno unavailable. The two genuine
source notes prepare private `@lapismd/lapis-server` 2.3.0 to 2.4.0. Only the
manifest, changelog and two consumed notes change. The npm lock graph, Cargo
inputs, dependency/audit policy, GitHub publication helper/workflow and approved
audit record stay unchanged. The receipt has `published=false` and
`writerAppMinted=false`. No synthetic release or npm/crate publication occurs.

## Owner workflow boundary

The pinned shared guard runs after the existing frozen install and before App
minting. It verifies installed CLI bytes, source recipe/config/formatter bindings
and the actual Changesets plan. Only `hasChangesets == 'true'` admits the
repository-scoped App and Changesets action. Invalid or missing output admits
neither branch. The version command, commit mode, writer permissions and
nonpersisted checkout credentials stay unchanged.

For qualified `hasChangesets == 'false'`, the existing `pnpm release:github`
helper runs with the workflow token through `GH_TOKEN`. This preserves genuine
post-version tag/GitHub release and image/CLI dispatch without minting the
version-PR App. The action's original publication configuration is retained.
No helper, runtime package, lockfile, Changeset or release version changes here.

## Regression and acceptance

`node --test scripts/version-pr-auth.test.mjs` initially reports two existing
passes and two failures: no pre-App runtime guard and no explicit empty-plan
publication branch. After the workflow change all four tests pass, preserving
canonical-main, scoped-token, fail-closed and publication assertions.

Direct acceptance of the owner's installed CLI and new qualification returns
`hasChangesets=true` for the genuine private source plan. An otherwise identical
disposable fixture with only an empty maintenance note returns false. Its first
setup attempt omitted repository context and CLI status refused to find main;
establishing the disposable baseline with Jujutsu corrects only that fixture.
No source note or main ref is moved for this check. Shared proof local tests
pass26 focused/285 total, separately from hosted whole-recipe execution.

Owner acceptance passes: `pnpm check` (zero Svelte errors/warnings, App audit and
four workflow tests), `pnpm test` (eight CLI/workflow cases and212 units),
`pnpm spec:check` (validate, mdBook, spec-first, App audit and workflow tests),
changed-file Prettier and Actionlint. Doctor reports10 passes and the existing
custom-alias warning; semantic QMD discovery succeeds. This workflow-only change
does not change application behavior or image inputs, so browser/Rust/image
requalification is not inferred from these source checks.

Hosted owner final-head/main guard acceptance and actual
release/tag, dual-architecture image/CLI publication and public runtime
acceptance are separate pending gates. Neither shared proof nor owner source
acceptance marks these delivery boundaries complete.
