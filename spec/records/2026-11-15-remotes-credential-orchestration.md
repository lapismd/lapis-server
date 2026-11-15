# Native credential mutation orchestration

Core now dispatches the existing generated save/replace/forget and device-flow
commands through backend-only credential actions. Providers own actual custody
and forge networking. The default action remains Unsupported; this change
alone neither installs a provider nor enables host routes. An asynchronous
resolution extension preserves existing synchronous resolvers while permitting
native forge refresh without nesting a runtime in the transfer executor.

Every accepted remotes operation holds an installation-wide process fence in
addition to its existing selection fence. Credential mutations reserve a durable,
secret-free observation epoch before provider effects. Each vault observes that
epoch before checking a request revision. A lost/partial provider response
therefore invalidates stale settings in every vault without replay. Epoch
rollback refuses; restored valid state becomes readable again. Replacement
preflight retains the host, username and credential kind. Tokens never enter
selection, epoch, journal or response records.

Started publication reviews retain their opaque credential references across
vaults, even after a source observation changes. Forget/replacement refuses while
that reference is retained. A new raw token can use a fresh reference; forge
completion that might replace an unknown account waits for pending recovery.
Native terminal outcomes/local stop retire the hold and consumed review; the
same review cannot replay. A failed preparation with no native pending receipt
also retires its consumed review, without discarding an uncertain native receipt.

## Baseline and focused results

- `repository_remote_credentials::credential_mutation_fences_every_vault_without_persisting_secret_inputs`
  initially failed once because SaveCredential returned Unsupported.
- Initial implementation compilation found two missing capability-metadata trait
  methods and one local binding shadowing the pending lookup. Importing the
  existing metadata trait and qualifying that lookup corrected all three errors.
- All three credential-policy regressions pass. They cover cross-vault stale
  refusal, scope-change rejection before custody/epoch reservation, stable
  replacement identity after a partial provider failure, forget, an independent
  process lock, epoch rollback/recovery, and accepted completion after cancelling
  the waiter and detaching its client. The completed write appears once to the
  next admitted client.
- A real native locally prepared backup intent plus an application hold proves
  that another vault cannot forget its credential. An old source witness does
  not discard protection. Explicit local stop clears native active tracking and
  the hold, permits forgetting, and never revives the consumed review. This is
  custody-policy evidence using a local prepared receipt, not real HTTPS/SSH
  authentication or actual host secret-store qualification.
- All 16 pre-existing affected settings/runtime/transfer cases also pass. Both real Core SSH
  scenarios pass after the process-fence and terminal-journal changes: original
  and backup exact publication, capture preservation, standby fetch and
  restart/lost-response observation/local stop remain qualified.
- The complete locked Rust workspace passes **853 tests, zero failures and nine
  opt-in ignores across 172 suites**. Real SSH opt-ins are separately exercised
  above. Escalation covers the shared native work fence, selection schema,
  credential provider contract and both publication-journal consumers.
- Strict all-target workspace Clippy and the complete workspace build pass.
  Generated Protocol bindings match Rust; complete Prettier passes. The first
  post-document-split spec-first run requested the mapped governance chapter;
  its root link and remotes topic now retain credential qualification, and all
  three specification lanes plus App ownership pass.

## Commands and runtime

All native commands use immutable tracked JJ archives, 2 CPUs/8 GiB and image
`sha256:6031c86d7c26d9e7011c958374462c600b10aa6a586ad7196d741756df0a5a32`.
The checked SDK pin remains merged jj-store `787dac1ae1ee524d54e4fa7fd4212cf99d5768f8`.
No path substitution, secret fixture in source archives or disk-reserve waiver
is used. The existing SSH harness retires its disposable endpoint/agent/keys.

- `node .local/remotes-validation.mjs test --locked -p lapis-app-core --test
repository_remote_credentials`
- `node .local/remotes-validation.mjs test --locked -p lapis-app-core --test
repository_remote_credentials --test repository_remotes_runtime --test
repository_remotes_transfers --test repository_remotes`
- `node .local/remotes-validation.mjs --ssh`
- `node .local/remotes-validation.mjs test --workspace --locked`
- `node .local/remotes-validation.mjs clippy --workspace --all-targets --locked
-- -D warnings`
- `node .local/remotes-validation.mjs build --workspace --locked`
- Node 24 `pnpm protocol:check`, `pnpm spec:check`, `pnpm format:check` and direct
  Rustfmt on the changed Rust files.

## Remaining work

Actual encrypted-server/OS-store provider composition, identity-checked forge
replacement, device sign-in startup, synchronization scheduling, HTTP/Tauri
adapters, production UI adoption and cold host acceptance remain open. Existing
native ciphertext tests and these policy fixtures do not qualify those hosts.
The final feature head still requires its own hosted checks and release gate.
