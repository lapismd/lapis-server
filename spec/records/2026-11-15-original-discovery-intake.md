# Original discovery and guarded-save intake

The native source pin `2f8f9ec484f02a8d46f08578f1d18302e0867cf1` is the exact
checked head merged through jj-store PR #82. Merge commit
`f0d01cedd1100c9c67639ca343b970b3497ac84e` has an identical source tree and
preserves that head and both separately verified commits in main. Hosted source
run 43690032688 passed every executable lane with same-attempt receipts.
The earlier temporary source qualification pin is now a merged source pin.

Core consumes the immutable native original inspector for passive settings.
Discovered names and sanitized optional URLs never choose origin or grant a
writer capability. Explicit save separately admits the native writer, holds the
private selection process lock through the Git config transaction, checks exact
discovered configuration and physical source, blocks pending publication, and
validates credentials before writing. Anonymous and host SSH selection work
with locked token storage. Endpoint/source mismatches cannot authorize an old
selection after a partial save. Network commands remain unimplemented and both
fetch/push capabilities remain false.

The original discovery baseline had one failing test (missing original target).
Four runtime cases then passed; an omitted generated TS declaration produced
one TS2304 error, fixed in the owning Protocol exporter. The guarded-save
baseline had one Unsupported failure. After implementation all five runtime
cases passed. Strengthening stale-save evidence exposed one IO-versus-Conflict
failure; a typed internal selection conflict preserves a safe public message
without exposing arbitrary IO details. The final focused impact surface has
15 passing cases across selection policy, accepted remotes runtime and physical
working-copy observation, with no failures or skips.

The complete Core/Protocol/Server run passes 800 cases, zero failed, seven
existing opt-in ignores across 155 suites, including the final conflict-code
refinement. All-target strict Clippy, native Server build, generated Protocol
check, configure, canonical specification/App ownership and whole-repository
Prettier checks pass. Changed Rust files pass direct Rustfmt. The unchanged
35-file inherited whole-workspace Rustfmt drift is recorded and reproduced in
[the preceding working-copy receipt](2026-11-15-original-working-copy-status.md).
No local source alias or package patch is used. The injected-only graph excludes
keyring and D-Bus dependencies.

Native checks run through the existing ignored revision-labelled archive helper
with Rust 1.99.0 and tool image
`sha256:6031c86d7c26d9e7011c958374462c600b10aa6a586ad7196d741756df0a5a32`:

- `node .local/remotes-validation.mjs test --locked -p lapis-app-core --test
repository_remotes_runtime --test repository_remotes --test repository_working_copy`
- `node .local/remotes-validation.mjs test --locked -p lapis-app-core -p
lapis-app-protocol -p lapis-server`
- `node .local/remotes-validation.mjs clippy --locked -p lapis-app-core -p
lapis-app-protocol -p lapis-server --all-targets -- -D warnings`
- `node .local/remotes-validation.mjs build --locked -p lapis-server`
- Node 24 `pnpm protocol:check`, `pnpm configure`, `pnpm spec:check`,
  `pnpm format:check`, and direct Rustfmt on the changed source files.

Local dependency locking used `cargo update --workspace`; it changed only the
18 jj-store source coordinates and removed unused OS keyring dependencies. An
initial remote-only generate-lockfile invocation did not copy an output back;
the tracked lockfile comes from the actual local update. Source and generated
TypeScript are qualified together.

This is still a source slice. Native transfer/review/publication orchestration,
credential-provider startup, HTTP/Tauri adapters, production UI adoption,
scheduling and both cold application acceptances remain open.
