# Hosted content API, MCP and plugin settings

Date: 2026-11-09. Owning requirement: SRV-SRV-014; shared contracts and controls
remain SRV-SRV-005/006/013. Jujutsu change `rwzwpopq`, on merged main `45f4c1b4` after native controls
`7dadbfcf` and the content specification extraction were merged with `gh stack`.
Rebasing onto that merge changed no product files.
This receipt qualifies the hosted source/process/UI slice, not Desktop, OAuth,
content writes, general MCP client products or CLI distribution.

## Behavior and ownership

The actual server mounts the shared read-only HTTP adapter at `/api/content/v1`
and stateless MCP at `/mcp`. Both use one shared native worker budget. A fresh
profile starts disabled. The exact public origin must satisfy the shared HTTPS
or loopback policy; incompatible origins preserve existing owner access while
leaving content unavailable. Vite preview/development forward both paths without
rewriting the public Host.

Owner controls under `/api/v3/content-access` retain existing protocol,
authentication and CSRF checks. Content credentials never authorize those
controls and owner credentials/cookies never substitute for a content grant.
Bodies are bounded to 8 KiB and at most eight controls are accepted concurrently.
Accepted work retains an owned task after disconnect and drains before shutdown.
Native vault admission is serialized against close/archive; whole-grant
retirement preserves unrelated grants. Shutdown permanently closes content even
if a dirty editor subsequently refuses native shutdown, while owner recovery
continues through the existing routes.

The control database adds only a boolean enable preference. Enable saves consent
before opening admission; disable fences/drains credentials even if storage
fails. Credentials are named, read-only, scoped to 1–20 active vaults and valid
for at most 24 hours. Only issuance returns the secret. Restart restores the
preference but never an old credential. Token-only hosts retain preferences only
in the process. Existing grant capacity, monotonic expiry and verifier-only
custody remain in Core; periodic host maintenance prunes expired grants.

The required Content API and MCP plugin registers a declarative settings section
through the public Notes API and option sources through its public host binding.
The existing Notes renderer owns the screen. The server-owned source provides
status/addresses, enablement, explicit vault selection, label/expiry, issuance,
revocation and one-time copying/dismissal. Credentials clear after one minute,
on disable/revoke or plugin teardown, and never enter App settings snapshots.
Draft labels preserve spaces while typing; native issuance trims and validates
them. No shared package implementation is copied or patched.

## Before/after evidence

- `cargo test -p lapis-server --test content_access --locked`: the original owner-control regression failed once (404 instead of 200). The exact target now passes seven real-socket cases: scoped HTTP/MCP byte parity, foreign-vault refusal, independent owner/content authority, CSRF, body/input bounds, expiry metadata, revocation, disable/re-enable, archive/close, durable preference/restart, incompatible origins and storage failure.
- `cargo test -p lapis-server --lib http::content --locked`: three lifecycle cases pass. Dropping a caller retains accepted control work until shutdown drains it; the eight-control budget refuses excess work before execution; a dirty native leave cannot revive content authority. A test-only `unwrap_err` Debug-bound compile error was corrected without changing production error types.
- `pnpm exec vitest run --project storybook --no-file-parallelism stories/ServerHost.stories.ts -t 'Content API and MCP'`: the initial missing-section baseline had two failures. Registration then passed both interactions but exposed two unhandled unknown-option-source errors; selecting the correct public host registry removed them. Expanded light/dark interactions initially had four accessible-name locator failures, then four draft-label failures. The exact final target passes all four desktop/narrow light/dark cases with zero unhandled errors.
- `pnpm exec vitest run --project unit web/src/content-access/model.test.ts`: five cases pass, including the real settings-controller persistence boundary, owner CSRF, timeout/dismissal/disable, late responses after disposal and sanitized errors without replay. The draft-label regression first failed once, then passed after preserving input spaces and accepting an empty draft until issuance.
- `pnpm exec playwright test -c web/playwright.hosted-notes.config.ts hosted-content-access.spec.ts`: three cold real-process journeys pass, covering bundled static UI, Vite preview and Vite development. Each uses real owner login and plugin settings, creates/revokes credentials, reads exact saved note bytes through HTTP/MCP, refuses a foreign UUID, checks browser storage and exercises enabled/disabled process restart. No preview-browser tools or visual baselines are used.

Initial host compilation reported four visibility/closure-capture errors, fixed
before the first passing HTTP run. Renderer type checks found one, then two,
invalid testing-library locator options; final type checks report zero errors.
These corrections did not relax product assertions or modify shared packages.

Final teardown review added `pnpm exec vitest run --project unit
web/src/content-access/plugin.test.ts`. Two initial harness failures established
that App construction needs a real Svelte component, not only a reactive root.
The production-plugin regression then failed once: its settings controller still
held the one-time token after unload. Registering source disposal before settings
registration clears the controller cache before unsubscribe. The exact regression
passes, and the full affected UI/catalog/hosted lanes are repeated for this fix.
No upstream contract change or internal-package API is needed.

## Full validation

- `cargo test --workspace --locked`: 778 passed, zero failed, seven existing ignored tests.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo build -p lapis-server --locked`: passed; cold browser journeys use the compiled server.
- `pnpm check`: zero errors/warnings, including the explicit App ownership audit.
- `pnpm test`: final run 105 passed, including the new plugin teardown regression (104 before that addition). `pnpm build`: passed.
- `pnpm test:storybook`: 50 interaction/axe cases passed; `pnpm build-storybook` and four cold `pnpm test:storybook:static` cases passed. Visual review remains pending, with no baseline changes.
- `pnpm test:hosted`: initial and final teardown-fix runs each passed 35 cases, zero failed and five existing opt-in skips. All seven canonical hosted groups completed; no new skips or relaxed assertions were introduced.
- `pnpm protocol:generate`, `pnpm protocol:check` and `pnpm configure`: passed. Generated owner-control DTOs are separate from bearer-callable content operations.
- `pnpm exec spec-validator doctor` and QMD search passed; doctor retains its existing script-wrapper warning. The installed validator is 0.2.0.
- Targeted Rustfmt, `pnpm format:check` and `pnpm spec:check` passed, including mdbook, mirrors/navigation, spec-first and ownership. Final receipt/verification edits repeat document checks only.

## Remaining delivery

Desktop must consume the shared native policy/adapters, own a loopback listener,
register its real plugin settings and prove native window/vault retirement and
credential custody. That application is not changed or qualified here. Content
writes and durable operation reconciliation, OAuth/public client compatibility,
Desktop discovery, persistent client approvals and CLI installation remain in
the owning content backlog. CI still owns both architecture image builds; local
hosted acceptance does not replace those checks or claim image publication.
