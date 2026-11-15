# Single-threaded native custody futures

The existing `jj-forge::accounts::http::Transport` exposes single-threaded
futures. Requiring Send on Core's credential future alias prevented an injected
provider from awaiting that SDK directly, despite Core constructing and polling
the future entirely inside its accepted native executor.

The alias now admits single-threaded futures; the provider itself remains
Send + Sync. No SDK future crosses the native worker/task boundary. Request
admission, process/selection fences, durable epochs and accepted-work ownership
are unchanged under SRV-SRV-004. This avoids a host-owned nested runtime or
copied forge implementation.

The strengthened cancelled-waiter test retains an Rc across its custody await.
It first fails compilation once under the Send bound. After widening that bound,
all 19 focused credential/settings/runtime/transfer tests pass, including
completion after waiter cancellation and client detach. Both real original and
backup SSH scenarios pass again. All-target strict workspace Clippy and the
workspace build pass, proving the compiled native consumers retain their task
boundaries. The preceding complete workspace run (853 passed, zero failed,
nine opt-in ignores) remains the broader algorithm/lifecycle evidence; this
type-bound slice does not claim a second full workspace test run.

Commands use the same immutable tracked archives, pinned worker image and Node
24 driver as the [credential orchestration receipt](2026-11-15-remotes-credential-orchestration.md):

- `node .local/remotes-validation.mjs test --locked -p lapis-app-core --test
repository_remote_credentials --test repository_remotes_runtime --test
repository_remotes_transfers --test repository_remotes`
- `node .local/remotes-validation.mjs --ssh`
- `node .local/remotes-validation.mjs clippy --workspace --all-targets --locked
-- -D warnings`
- `node .local/remotes-validation.mjs build --workspace --locked`
- Direct changed-file Rustfmt and canonical specification/format checks.

Actual provider composition, identity-checked token replacement, device-flow
startup and both admitted hosts remain separate work. No renderer or Protocol
contract changes in this slice; no real stored credential is read or written.
