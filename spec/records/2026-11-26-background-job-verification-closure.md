# Background-job v1 verification closure

This receipt closes the stale consumer gates for SRV-JOB-001–008. It changes
no executable contract. Previous receipts retain their original failures and
then-pending gates.

- Server controls and terminal cancellation are merged in
  [PR157](https://github.com/lapismd/lapis-server/pull/157), merge
  `2c93d58b3477ee826a4b1faac1958fc5e82bab16`. All eleven hosted check contexts
  passed, including Rust tests, production catalog and Required Server CI.
- [Registry adoption](2026-11-25-background-registry-adoption.md) records
  frozen released-package resolution, 241 browser units, 110 Storybook
  interaction/accessibility cases, ten static journeys and 53 enabled hosted
  cases. The six existing optional/native skips remain explicit.
- [Terminal completion](2026-11-25-plugin-job-completion.md) records two
  failing-before/passing-after regressions, ten execution, five scheduling and
  ten Core native-service tests, strict Clippy and the full Rust retry.
- Desktop [PR68](https://github.com/lapismd/lapis-desktop/pull/68) merged at
  `5da6fd481d9796518ef88b44f874d3d902b78184`. Its tree equals the reviewed
  `f01152187a567bd7da0bcfcbc5c48410baa99366` tree. Exact-head CI45570627327
  and full qualification45571406013 passed. Eleven actual GTK/WebKit native
  cases qualified metadata/Tasks defaults, guarded edits, retained hidden work,
  cold restoration, explicit vault close and Quit drain. Source suites passed
  975 Node, 749 AI and 295 Rust executions; catalog passed 2180 cases.
- The unchanged signed Tasks0.4.0 archive was exercised in both real hosts:
  pagination over101 tasks, one persistent notification/checkpoint, cold
  recovery and planner completion. Archive SHA256
  `04953bda8012f50263ac2224a649a64d0892a43b0a70cd6d92132985af2eee3b`,
  size701741 bytes. Registry PR210/211 publication merged at
  `9683edd4549b5529441761e82a00ac6df78404e1`; production deployment45580060837
  passed. Omarchy verification at2026-11-25T08:55:01Z checked the live index,
  revocations, signed Tasks metadata and downloaded archive against the trust root.

Executable source and consumer qualification ran in Docker context `omarchy`.
macOS build jobs establish buildability only. Actual macOS system-alert permission
and delivery acceptance remains a Desktop gate; the shared native sink and
persistent notification history do not establish an OS banner appeared.

Server2.10.0 distribution, Desktop rc.5 distribution and the separately requested
network/credential extension have independent receipts and are not claimed by
this v1 closure. Beancount production banking remains owned by its native service
plan, including opaque credentials and revision-checked ledger imports.

Documentation qualification used `pnpm spec:check` and `pnpm format:check` on
Omarchy. The first attempt reported one missing governance mapping; the second
reported one governance file-size violation. Replacing its stale job-status
sentence with the closure link satisfied both existing limits. Source
`90100800e5cd2c14051a6b0e8b99a10dab2d2930` passed both complete lanes, with zero
remaining findings. The final receipt-only addition is checked again before
commit. No product test, assertion or specification limit changed.
