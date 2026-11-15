# Native Sync account transport — 15 November 2026

## Boundary and before state

Server core change `qyvwnxwu`, based on merged main
`6910cdf32bea2817350b3670d28e5b0bb72250c9`, implements SRV-SYNC-015.
Before implementation, the exact origin-admission target could not compile:
`lapis_app_core::sync::account` was absent (one unavailable target).
After implementation, the same origin-admission target passes as part of all
10 focused transport cases. No test skip or assertion was weakened.

## Behavior

The shared native client binds account initiation, S256 redemption, managed-vault
discovery and revocation to a validated origin. HTTPS is the default; plaintext
requires an explicit loopback development policy. Credentials are bound to that
origin before request construction. Requests do not acquire browser cookies or
ambient authorization, redirects and library retries are disabled, and errors
contain no URLs, response content, credentials or paths.

The connection/redeem/sign-out response limit is 4 KiB; discovery is 2 MiB and
4,096 unique vault identities. Requests have a five-second connect bound and a
ten-second total bound. Device IDs, consent URLs, names, discovery scope and
credential expiry are checked. Expiry permits one minute of server clock skew.
Proofs, bearers and response buffers use zeroizing host-owned storage; secret
wrappers implement neither Debug nor Serialize. HTTP library request storage
remains host-owned. This transport does not persist credentials or replay any
uncertain accepted mutation: the protected service must own those decisions.

## Exact and widened verification

- Before:
  `CARGO_TARGET_DIR=.local/rust-core-target cargo +1.99.0 test --locked -p lapis-app-core --test sync_account_transport account_origin_requires_https_or_explicit_loopback_and_no_ambient_authority`:
  unavailable with one unresolved-module compiler error.
- After: `cargo +1.99.0 test --locked -p lapis-app-core --test sync_account_transport`:
  **10 passed, 0 failed, 0 ignored**. These exercise origin/proof admission,
  foreign consent URLs, redirect bearer isolation, chunked response overflow,
  credential scope/client/expiry, protected origin restoration, duplicate and
  malformed vault entries, lost acknowledgements without replay, discovery
  overflow and an actual stalled-response ten-second deadline.
- `cargo +1.99.0 test --locked -p lapis-server --test sync_native_account_transport`:
  **1 passed, 0 failed** against the real current HTTP server/control store.
  It requires owner consent, redeems the native S256 proof, restores a credential
  from host-only fields, discovers a real managed vault, proves ordinary vault
  routes still refuse that credential, signs out and proves discovery refusal.
- The first real-server fixture run was **0 passed, 1 failed** because the
  existing insecure-browser policy accepts only `http://localhost`, while the
  fixture selected a numeric loopback origin. The fixture now uses localhost;
  production origin policy is unchanged. The exact rerun above passes.
- `cargo +1.99.0 test --workspace --locked`: **855 passed, 0 failed, 7 ignored**
  across 172 target result summaries. Existing ignores are five ONNX asset
  lanes plus two isolated crash-process helpers; no Sync case is ignored.
- `cargo +1.99.0 clippy --workspace --all-targets --locked -- -D warnings`:
  passed. Changed Rust files pass pinned rustfmt; canonical specification and
  App-ownership checks pass. macOS's existing large-unwind-table linker warning
  remains visible; it is not a functional failure or a Linux platform result.

All Rust commands use the task-owned `.local/rust-core-target`; workspace test
and Clippy logs remain in `.local/sync-native-account-*.log`. Local source and
real-HTTP evidence is macOS ARM64, Rust 1.99.0. It does not qualify Tauri IPC,
Keychain, profile/vault/root admission, cold protected restoration, endpoint
membership, content keys, destination transfer, native windows or release.

## Review and remaining boundaries

The client adds no account bearer or verifier to the browser consent screen and
no application/provider fallback. All existing consent, object, journal,
ancestry and arrival repositories remain independent. Shared protected account
orchestration must durably retain intent and unknown outcomes; Desktop supplies
OS custody and revalidates its admitted window/profile/vault/root across waits.
No plugin activation, package publication, user-vault mutation, deployment or
installed-application replacement was performed for this source slice.
