# Background-service registry adoption

The Server consumes released API0.6.15, Workspace0.4.11 and App Profile0.3.1
through normal npm ranges and frozen registry resolution. The reviewed source
receipt admits only the exact cohort and the API's supported i18next26 peer.
Other peer edges, immutable registry identities, canonical prose and scope
checks remain enforced.

## Remote qualification

All executable source acceptance used Docker context `omarchy`.

- Frozen registry installation, dependency-intake suite, validator doctor,
  specification and formatting passed.
- The complete quality and unit lanes passed: 241 browser units across 41 files.
- All 110 Storybook interaction/accessibility cases and the static build passed;
  the 10 static browser journeys passed.
- `pnpm test:hosted` passed all 53 enabled browser cases. Its existing six
  declared native/optional skips remain unchanged. The plugin-operations test
  also passed.
- Both exact session-expiry cases passed three cold real-clock repetitions each
  (six passes), followed by the complete 13-case host suite and all later host
  configurations.

The final successful registry/clock source was
`b93249bd5ff9ed8e16870067f1932b21d258c86e`, pack SHA256
`128d64ef487d91edd8f282044f268bd781d86cef9225133ab2cd54910d6f8ecb`,
container `lapis-plugin-jobs-server-registry-20261125f`. Logs retain
`.local/plugin-jobs-server-registry-{final,clock-regression}.log`.

## Preserved failures and subsequent integration

The first harness built Cargo outputs into a shared cache without the expected
`.local/rust-core-target` link. A separate retry also omitted Vite's output.
Both failed setup attempts remain logged. The corrected isolated worker uses
that cache link and builds production assets before browser acceptance.

The full real-host run then reproduced the existing 403 expiry failure: the
Sign in heading did not appear within its original five-second assertion. Its
fixture had advanced all browser timers by30 seconds while real network probes
had five-second abort timers. Expiry acceptance now waits for the actual native
heartbeat, preserving the original deadline, real server rejection, no-reload
check and workspace teardown assertions. No production timeout was changed.

Current main independently merged the mobile vault target work at
`981dc3d0da9a8097cb2adce1a12099588339df15` after the successful cohort. The
source-bound registry receipt is rebased to that exact parent; the composed source
`bf95c38160d1b1d03c64b307ac8c5081ae6b109a` passed frozen install, source checks,
units, complete workspace Rust tests and strict Clippy. Its freshly built native
server also passed both expiry cases and plugin operations. Hosted CI remains
required for the reviewed head. Desktop pins,
signed Tasks installation and system-specific notification delivery remain
separate gates. No test assertion, audit policy or visual baseline was weakened.
