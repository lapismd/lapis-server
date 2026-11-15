# Encrypted forge credential custody source

Change `oypzrtwr` extends SRV-SRV-015/016 to the forge manager's opaque
credential JSON. The existing raw-token encrypted format stays compatible.
Account metadata remains separate; this slice does not initialize a manager,
register routes, enable remote actions or qualify either cold application.

The installation-private store has a separately bounded `.forge` namespace.
AES-256-GCM authenticates a distinct forge domain, installation UUID, owner UUID
and the full 256-bit account identity. Records do not truncate account hashes or
invent an endpoint. The native process lock and operation mutex cover both
namespaces. Raw-token inventory excludes forge records; account inventory will
remain the forge manager's responsibility.

Reads, replacement and deletion require valid owner/account identities and
available custody. Ciphertext authentication failure locks the custodian. The
key verifier cannot be recreated over existing forge-only ciphertext. Secret
inputs/outputs use zeroizing buffers and have bounded lengths. Credentials
remain outside repository configuration, vault contents and browser data.

The isolated baseline command
`node .local/remotes-validation.mjs test --locked -p lapis-server --lib
credentials::forge_tests` compiled the immutable pre-implementation snapshot and
reported 18 E0599 errors for the missing encrypted forge methods. Its two initial
regressions cover encrypted restart/full-account identity and replacement/
forgetting/locked storage. A third regression added during review proves that
losing the verifier never rekeys existing forge ciphertext. The final first case
also preserves an independently scoped raw token across forge writes/restart.

Validation uses the isolated, revision-labelled Docker snapshots described in
the [selection receipt](2026-11-15-remotes-selection-policy.md), Rust 1.99.0,
Node 24.10.0 and pnpm 10.34.5:

- Exact baseline command rerun after implementation: 3 passed, zero failed.
- `node .local/remotes-validation.mjs test --locked -p lapis-server --lib
credentials`: all 13 token/forge custody cases pass, zero failures.
- `node .local/remotes-validation.mjs test --locked -p lapis-server`: 161 passed,
  zero failed or ignored across 29 suites, including real-socket host admission,
  origin/CSRF, detached work, restart and shutdown cases. These existing host
  cases do not qualify the unregistered remote service.
- `node .local/remotes-validation.mjs clippy --locked -p lapis-server
--all-targets -- -D warnings`: pass.
- `node .local/remotes-validation.mjs build --locked -p lapis-server`: pass.

Next: inject this backing through the owning forge SecretStore contract, wire
operator key provisioning/startup and authenticated host remotes, then qualify
real sign-in, scope resolution, native transfers and cold lifecycle behavior.
No plaintext fallback, public crate publication, visual baseline update or
application release is introduced.
