# Native Server terminal profile

Date: 2026-11-20. Requirements: SRV-HST-020 and SRV-GOV-001/004.

The user reported missing shell styling in `350d8e7-slim` (Server v2.5.1).
That release contains the font/renderer/session UI from PR #111; its coloured
prompt/listings were Storybook fixtures. This follow-up implements the missing
native application policy. The user explicitly requested relevant app tests
without Docker-image verification; no Docker run or image acceptance is claimed.

## Ownership and integration

Terminal Host [PR #5](https://github.com/lapismd/terminal-host/pull/5) adds only
`ShellCommand` and `PtyService::with_command`: host-selected argv/environment,
absolute program validation and unchanged required terminal identity variables.
Its original constructor remains login-interactive. The private crate is pinned
by exact public Git revision `88fae24eb24934bb8dcc8bdf2eb0f1e86ffb39ef`.
No npm dependency, package patch, source alias or local Cargo override is used.

Core exposes an optional per-vault shell factory with the admitted physical root
and UUID. Server's common Host constructor installs its own policy before vault
admission, covering both configured and managed/database startup. Other hosts and
Core's default retain the old shell behavior. Desktop source is unchanged.

The compiled application embeds the startup script in its host-owned first
`PROMPT_COMMAND`. A real login-interactive Bash loads normal system/user startup
files once, then preserves user prompt hooks and displays cyan paths, a magenta
eight-character admitted vault UUID and green/red status. GNU `ls --color=auto`
provides actual coloured output; macOS native development has a listing fallback.
No script is typed into the PTY or written into user configuration. The initial
bootstrap replaces itself; subsequent hooks preserve the user command's status.

UUID shortening applies only to the admitted managed vault root and its children.
Foreign UUIDs stay full. Prompt variables expand cwd bytes once as data; control
characters are sanitized for display. Real cwd, HOME, `pwd`, filesystem paths,
stream cursors and session IDs are not rewritten. The opt-out
`LAPIS_SERVER_TERMINAL_PROFILE=0` is read when constructing the Server Host and
restores the original unmanaged login shell.

## Before and focused application evidence

- `cargo test -p lapis-server --test terminal_profile --locked`: before the
  application change, one failure/zero passes, showing the real plain prompt.
  After the final implementation, one passed/zero failed through `Host::new`
  and `Host::with_profile`, with the profile enabled by default. The test starts
  fresh child test processes with isolated HOME/history.
- `cargo test -p lapis-server --lib terminal_profile::tests --locked`: seven
  real-PTY cases pass on system Bash 3.2. Repeating with
  `LAPIS_TEST_BASH=/opt/homebrew/bin/bash` passes all seven on native Bash 5.3,
  exercising native array prompt hooks as well as scalar hooks.
- Those cases cover actual ANSI prompt/listing bytes, directory and executable
  colours, success/failure and retained `$?`, one-time startup files and hook
  order, unchanged login-interactive mode, opt-out, literal malicious cwd text,
  unshortened foreign UUIDs, full `pwd` output and nested interactive Bash.
  Startup files remain unchanged.
- `cargo test -p lapis-app-core --test service_terminal --locked`: three passed;
  client isolation, dirty-close retention, detach/close fencing, root/shell
  refusal and input bounds remain enforced.
- `cargo test -p lapis-server --test terminal --locked`: four passed through
  actual authenticated/CSRF-protected HTTP and finite output streams, including
  owner revocation without retiring another client's terminals.
- Upstream: the new contract target initially reported three missing-API compiler
  diagnostics, then three passed. All eight engine PTY tests, strict Clippy,
  Rust-only audit, formatting, build, mdBook and hosted source CI pass.

Intermediate failures remain explicit: the first initializer left the Host
regression failing and five of six native cases failed because its process-
substitution startup did not execute. The inline initializer resolved that;
one hook-order case then exposed missing first-prompt callbacks. Running the
composed initial hooks initially made five cases show a red initial prompt due
to the bootstrap conditional's exit status. Explicit initial status resolves
that, with zero remaining focused failures. Canonical physical root/home display
comparisons handle macOS `/var` aliases without changing HOME or real paths.

The nested-shell regression initially failed once because the environment-based
bootstrap left its composed private hooks exported. Removing that export after
composition lets child shells load their own prompts without missing-function
errors; user hooks still run in order in the owning Server shell.

The local macOS linker emits its existing large `__eh_frame` warning for native
server binaries. Strict Clippy passes with warnings denied. The source and app
checks remain distinct from any later Server release or image qualification.

The first canonical check reported two acceptance-detail diagnostics (five bullets and a multi-sentence bullet). Moving the native-policy context into the requirement paragraph preserves the existing four atomic acceptance bullets and the unchanged validator rules.

## Broader application checks

- `cargo test -p lapis-app-core -p lapis-server --locked`: 877 passed, zero
  failed, 11 existing ignored asset/SSH cases and child-process helpers. The final
  seven-case profile target and both production Host startup branches also pass
  after the nested-shell export fix.
- The directly dependent protocol, CLI, Content HTTP and Content MCP packages
  pass their complete locked test suites: 95 passed, zero failed or ignored.
  Core/Server and all four dependents
  pass strict all-target Clippy and locked native builds.
- The normal frozen registry install, `pnpm check`, `pnpm test` (227 unit cases
  plus tooling), `pnpm build`, `pnpm test:storybook` (92 interaction/axe cases),
  `pnpm build-storybook` and `pnpm test:storybook:static` (five cases) pass.
- `pnpm spec:check`, `pnpm format:check` and formatting checks for the new Rust
  profile/test files pass. Spec Validator 0.2.1 is current. Node 24 QMD doctor
  reports 16 passes with the existing script-alias warning; QMD search succeeds.

This is application/source qualification for a review PR, not publication. No
Docker verification was run for this follow-up, as requested.

## Linux CI prompt metadata correction

Server PR #115's first Rust lane reported 37 passed and one failed in the Server
library. Linux Bash emitted an OSC 3008 shell-metadata sequence containing its
full physical cwd, followed by the correctly shortened coloured prompt. The
assertion incorrectly searched the entire transport frame for the full UUID.

The exact failing prompt/status target reproduces locally with a real startup
hook emitting equivalent OSC metadata: zero passed and one failed before the
test correction. It now checks the complete visible ANSI prompt for the exact
compact path and success arrow, while requiring the hidden metadata to retain
the full physical cwd. Runtime code and terminal metadata are unchanged.

After correction, the exact target passes and all seven profile cases pass on
Bash 3.2 and 5.3. The complete locked Server suite passes 198 cases with zero
failures and one existing SSH opt-in. Strict Clippy, native build, specification,
Prettier and the changed Rust test file's formatting checks pass. Terminal Host
#5 is merged; its tested exact dependency revision remains on main unchanged.
