# Managed backup native transfer orchestration

Core dispatches the existing generated target-aware remote commands to the
managed backup. Host admission remains explicit; default compositions expose no
network authority. Fetch imports metadata through the actual native capture
worker, or through a standby native Session without starting capture. Immutable
review pins the existing recorded checkpoint, ancestry and native publication
intent. Later capture cannot add saved files to an already reviewed publication.

Backup and original journals have independent source/connection bindings and
share bounded capability-relative record IO. The original journal format is
unchanged. Missing backup history retains unknown counts and disables push.
Invalid review/request identities refuse before credential resolution. Native
receipts retain stable identities across restart; observe does not republish and
local stop needs neither a credential nor transfer admission. New connections
and local captures leave automatic synchronization disabled.

## Narrow failures and recovery

- The initial real backup SSH case failed once because transfer capabilities
  remained false. The implemented backup projection admits only the configured,
  supported target and requires actual recorded history for push.
- Initial compilation exposed six import/type errors and then one wrong public
  method name. Imports now use the public vault-workspace types and standby
  metadata import calls `Session::import_fetched`.
- Adding standby/stop coverage exposed two test compilation errors: public DTO
  enums do not implement PartialEq. Variant assertions preserve the intended
  checks without changing DTO contracts.
- The first completed stop assertion failed once because it expected an active
  Stopped item. Native `Binding::stop_publication` explicitly retains the named
  terminal receipt and clears active tracking. The corrected case proves no
  active item, refusal to replay the consumed review and no additional receive.
- Both real SSH scenarios now pass, original first and backup second against
  the same disposable endpoint. Backup proof covers save/capture without push,
  exact reviewed checkpoint after later capture, later-file exclusion, fetched
  counts, lost-response restart observation, standby fetch without capture and
  local stop after restart with transfer admission disabled.
- All 37 focused selection/runtime/working-copy/transfer/backup lifecycle cases
  pass. The new no-capture case retains unknown counts, initializes no repository
  and refuses invalid identities without resolving a token.
- The complete Core/Protocol/Server suite passes 803 tests, zero failures and
  nine opt-in ignores across 158 suites. The two SSH opt-ins are separately
  exercised above. Escalation covers shared private-journal IO, backup capture
  lifetime and both target consumers. Strict all-target Clippy and the native
  Server build also pass; changed Rust files pass direct Rustfmt.

## Commands and runtime

Native commands use immutable tracked JJ archives in the remote Docker worker,
2 CPUs/8 GiB, Rust 1.99 and image
`sha256:6031c86d7c26d9e7011c958374462c600b10aa6a586ad7196d741756df0a5a32`.
The SSH harness runs Node 24.10.0 with disposable keys, agent, strict known-host
file and bare Git endpoint. Host credentials and trust remain untouched.

- `node .local/remotes-validation.mjs --ssh` runs both actual transport cases.
- `node .local/remotes-validation.mjs test --locked -p lapis-app-core --test
repository_remotes_transfers --test repository_remotes_runtime --test
repository_remotes --test repository_working_copy --test backup_commands --test
backup_selection --test service_backup`
- `node .local/remotes-validation.mjs test --locked -p lapis-app-core -p
lapis-app-protocol -p lapis-server`
- `node .local/remotes-validation.mjs clippy --locked -p lapis-app-core -p
lapis-app-protocol -p lapis-server --all-targets -- -D warnings`
- `node .local/remotes-validation.mjs build --locked -p lapis-server`
- Node 24 `pnpm configure`, `pnpm protocol:check`, `pnpm spec:check` and
  `pnpm format:check` pass. Spec doctor passes with the existing script-alias
  warning; QMD `pnpm spec:search "backup remotes"` succeeds. The initial search
  with an extra `--` was rejected by the CLI and corrected.

## Intake and remaining gates

The earlier original observer intake at `a1d5efa0d96024075e3971f278db76684c49cb18`
is merged through jj-store PR #83, merge `a7a1df3d4f79ec4cca0452647e46cbc2dda4ae09`.
The checked and merged trees match; its complete required hosted CI passed.

This backup slice pins jj-store
`787dac1ae1ee524d54e4fa7fd4212cf99d5768f8`, PR #85, which supplies immutable
HistoryReader review/counts and capture-independent transfer. Required hosted CI
run `43701293966` passed, including both backend shards, four Rust shards,
browser/Storybook jobs and its same-attempt required gate. PR #85 merged at
`6fdce65c841216b07024ce7ea2cb822f9474a763` with the checked head and identical
tree, fulfilling the merged SDK intake gate. Credential mutation/sign-in, native
scheduling, operator startup, both host adapters, production UI adoption and
cold host acceptance remain open. This receipt does not qualify an installed
host or the complete feature release.
