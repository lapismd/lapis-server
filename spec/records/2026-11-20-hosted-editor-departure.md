# Hosted concurrent editor departure

20 November 2026. SRV-GOV-002; PR #116 release qualification.

The version PR only consumes a changeset, updates the changelog and changes
2.5.1 to 2.5.2. Its renderer job 136418577386 in run 44773878803 reports
14 Tasks cases passed and one failed. The failure is the concurrent dirty-editor
case, where the server correctly refuses shutdown with retained editor state.

The retained Playwright trace shows every scenario assertion passing: the draft
is saved exactly, the first Task submission is refused, the owning browser's
submission completes, editing generations rearm and closing the editor removes
its Hosted.md claims. The fixture then closes the second browser frame directly
without awaiting native client DELETE, before logging out the first browser.
Both browser pages also share one account session.

The original exact command is `pnpm exec playwright test -c
web/playwright.rust-tasks.config.ts web/e2e/rust-server-tasks-editors.spec.ts`.
An unchanged warm run passed one case, which does not waive the CI failure.
The regression additionally requires a successful release of the second native
client before its frame closes. The fix belongs in this private test's owned
lifecycle, without changing runtime policy, timeouts or the shared fixture.

The strengthened exact target fails zero passed/one failed on the old lifecycle:
the observed order is only `closed`, with no successful native release. Its
original assertion is retained in the failure attachment; the old diagnostic
callback additionally tries to inspect the already closed page. That error is
also retained rather than replacing the before evidence.

The repaired target passes one case. Its independently authenticated secondary
App crosses guarded logout, waits for successful client DELETE, then closes its
frame; the exact order must be `released`, `closed`. A further native command
through the original browser must still succeed after that logout. Every
original draft, conflict, generation, no-retry and clean-shutdown assertion
remains. Existing `withHostedCleanup` now preserves scenario errors while
attempting every owned release, and diagnostics avoid reading a closed page.

The change is applied on PR #116's existing version commit, retaining its
2.5.2 metadata. This private test repair adds no production behavior or changeset.

## Final local qualification

- The final exact regression passes one case on a fresh owned Rust/Vite host.
- With a fresh 2.5.2 renderer build, all 15 Tasks and both Properties browser
  cases pass with zero skips or failures. Original runtime/plugin-error checks
  and the existing 15-second clean-shutdown deadline remain unchanged.
- Type/Svelte checks report zero errors or warnings; App ownership and release
  tooling checks pass. All 227 unit cases and the owning tooling tests pass.
- The 2.5.2 production web build passes with existing bundle-size advisories.
  Canonical specification and formatting checks qualify the final evidence.

One earlier local cohort was invalidated by editing the test source during
Playwright trace collection: it reported an unexpected source-stream byte count
and truncated ZIP, and was interrupted. Its artifacts remain under
`.local/terminal-preview/pr116-invalid-cohort-artifacts`. The complete cohort
was then rerun from unchanged sources with the passing results above; that
invalid run is not treated as product evidence or a passing check.

Spec Validator 0.2.1 is current; Node 24 QMD doctor passes 16 checks with the
existing script-alias warning, and semantic specification search succeeds.
This qualifies application test infrastructure; no image run or visual baseline
update is included. Hosted CI must still qualify the pushed PR head.
