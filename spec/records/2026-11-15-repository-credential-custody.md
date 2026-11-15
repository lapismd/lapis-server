# Repository credential custody: native Server adapter

Source change: `swtrxykt`, based on remote main `2dd27c10`.
Requirements: SRV-SRV-015/016 and existing SRV-HST-003/004.
This is a source foundation for the approved remotes plan, not hosted adoption.

## Behavior

The installation-private adapter stores AES-256-GCM ciphertext with a fresh
96-bit nonce and authenticated format/installation/owner/credential identities.
Its separately supplied operator key must be a regular file of exactly 32
bytes and cannot be inside the credential directory. The directory must be
private. A missing, incorrect or malformed key, missing verifier with existing
records, or failed verifier keeps custody locked without rewriting ciphertext.

Native inventory excludes tokens. Resolution checks the authenticated owner and
approved HTTPS repository endpoint or explicit origin/account scope. Replace
preserves the opaque credential ID;
forget removes the encrypted record durably. Transient secrets use zeroizing
buffers and have no inventory or native-result serialization implementation.
Capability-relative Core file operations retain atomic publication and physical
root checks. One process lock and one serialized native operation guard prevent
competing custodians and capacity races. No request chooses a filesystem path.

The [operator notes](../../docs/repository-credentials.md) describe separate
mounts/backups, persistence, restart, recovery and unsupported in-place rotation.
The adapter is not registered with startup, HTTP or the Repository History UI.

## Validation

All Cargo commands use `CARGO_TARGET_DIR=.local/remotes-target`,
`CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`, and dev/test debug information off.
Tests use disposable private directories and synthetic credentials only.

- Before implementation: `cargo test -p lapis-server --lib credentials
--locked` passed with **0 selected / 11 filtered** tests. This confirms missing
  coverage, not working custody.
- Initial implementation compiled after correcting three type/API errors in
  capability permissions and byte-reader arguments. No diagnostic was suppressed.
- Exact `cargo test -p lapis-server --lib credentials --locked`: **10 passed**.
  Coverage includes encrypted restart persistence, secret-free inventory,
  durable replacement/forgetting, missing/wrong/invalid keys, installation/owner/
  record binding, endpoint/account isolation, tampering, encryption-key separation,
  exclusive native ownership and concurrent capacity enforcement.
- Complete `cargo test -p lapis-server --lib --locked`: **21 passed**.
- `cargo clippy -p lapis-server --all-targets --locked -- -D warnings`: pass.
- `cargo build -p lapis-server --locked`: pass; the macOS linker reports the
  existing large `__eh_frame` warning, with no compile errors.
- `pnpm exec spec-validator doctor`: **10 passed**, existing script-alias warning.
- `pnpm spec:check`: validation, mdBook, mapped spec-first and App-ownership pass.
  Initial new-requirement detail and 32 KiB verification-page failures were
  corrected through atomic acceptance bullets and a separate verification topic,
  with its metadata-only Storybook mirror and navigation.

Rust/Cargo 1.99.0, Node 24.10.0, pnpm 10.34.5 and Deno-free spec-validator
0.2.0 were used on local macOS ARM64. These are source checks, not Linux image,
Docker, native Desktop, browser or live provider qualification.

## Remaining qualification

Shared Core/Protocol remotes policy, actual host wiring, authenticated owner/
vault/origin/CSRF/generation checks, credential references in private connection
state, GitHub/Forgejo integration, real HTTPS/SSH transport, cold restart/detach/
vault-close acceptance and export/log redaction remain required. The new
verification rows stay Partial. No release, installed-app replacement or user
profile modification is qualified by this source receipt.
