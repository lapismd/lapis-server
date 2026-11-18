# Hosted Tasks failure ownership

Main Server CI44206370737 at
`c0bcecb5860030a118996bb9714a5d92aa03c931` reports one failed case among15:
`another App saving ordinary settings preserves interrupted native Task references`.
Its final error is the30-second Username assertion in
`rust-hosted-session.ts`, called by the Tasks fixture's cleanup. The scenario's
catch had already attached its page and rethrown an earlier failure, which the
nested finally chain overwrote. The page retains the vault picker and a
prepared workspace. This establishes error masking, not the initiating runtime
cause. The successful PR run44198385747 and failed merged main have the same
Git tree `05031aafe5805ec3f43bc4f730d0aa6ec2c85fb7`.

The owning source regression reproduces the existing nested-finally behavior:
three of six cases fail before repair. After repair, six of six pass. The
helper retains the original error, diagnostic errors and every independent
cleanup failure in encounter order. A lone failure retains its exact identity;
multiple failures retain the first as their AggregateError cause. All cleanup
attempts still run; cleanup alone still fails the test. The recovery scenario
also attaches its original stack before teardown. Fixture runtime/plugin and
native recovery assertions are unchanged.

The existing CI upload retains only `test-results/rust-hosted-tasks/` after a
failure for seven days. This includes Playwright traces, error contexts and
attachments. It does not upload `.local` fixture profiles, bootstrap-password
files, private keys or deployment credentials. Previously only unrelated Mod
loader artifacts were retained, so the initiating failed-run error cannot be
recovered from its truncated console preview.

Local qualification rebuilds the real Rust server and CLI and the public Vite
renderer before the exact cold single-case command:

```sh
pnpm exec playwright test -c web/playwright.rust-tasks.config.ts \
  web/e2e/rust-server-tasks-recovery.spec.ts \
  -g 'another App saving ordinary settings preserves interrupted native Task references' \
  --workers=1
```

The exact case passes1/1 in29.3seconds with fresh service/profile/browser
fixtures. This is not evidence that the original hosted runtime failure was
fixed. The broader unchanged-source Tasks cohort and required source checks
are recorded separately below before delivery.

## Complete source qualification

The complete directly affected Tasks cohort passes15/15 in3.3minutes, including
the original recovery case after its six preceding cases. `pnpm check` reports
zero Svelte errors/warnings and passes application ownership plus four release
workflow cases. `pnpm test` passes25 dependency-intake cases, eight CLI/release
cases and218 Vitest cases across37 files. The required Rust server/CLI build and
Vite build pass; complete specification validation, mdBook and exact-main
specification-first pass. Doctor, QMD discovery, changed-file formatting and
workflow actionlint pass. No model/provider or release/image qualification is
claimed by this targeted maintenance slice.

The earlier main runtime failure is not reproduced by either cold local run.
This change repairs proven failure ownership and artifact retention only;
fresh PR/main hosted checks remain mandatory, with a future failure retaining
its original stack and complete trace. No product fix, new quarantine, retry,
assertion relaxation, dependency update or source advisory exception is added.
