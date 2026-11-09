# Scoped native content reads

Requirement: SRV-SRV-007. Parent contract slice: `aa2940c2`, PR #19.

The shared Core now owns a read-only content session constructed from dedicated
native vault attachments. It provides capability discovery, admitted vault and
Markdown-note listing, saved UTF-8 content reads and SHA-256 versions. It retains
revocation/expiry, physical admission, bounded reads and session/query/snapshot
cursor ownership. New mutations remain refused and no transport is enabled.

## Validation

Environment: Linux, Rust 1.99.0, pnpm Node 24.19.0 and system JJ 0.46.0.
CI retains its independent pinned JJ 0.45.1 and native/hosted gates.

- Before implementation, `cargo test -p lapis-app-core --test content_reads
--locked` failed to compile because the shared content module did not exist.
- The same focused target then passed 5/5. It covers authoritative/versioned
  reads, write refusal, foreign vaults, revoke/expiry/close, pagination isolation,
  changed listings, symlink escapes, replaced roots, invalid UTF-8 and size limits.
- `cargo test -p lapis-app-core --lib services::content::tests --locked` passed
  2/2: revocation while a reader waits on the native vault lock, and bounded
  continuation retention with explicit eviction conflicts.
- Strict workspace Clippy passed. Protocol drift and TypeScript client checks
  passed without changing generated artifacts.
- Specification validation passed after adding the required mapped governance
  confirmation; the preceding attempt failed that mapping. Formatting passed.

`cargo test --workspace --locked`: 717 passed, 0 failed, 7 existing
model/helper ignores. No new ignore or assertion waiver was introduced.

## Remaining boundaries

These are native source tests, not external API or Desktop acceptance. Hosts
still own authentication, credential/grant persistence, session cleanup and
blocking-worker placement. Durable mutation receipts, editor-safe writes,
HTTP/CLI/MCP adapters and actual-host integration stay in the content backlog.
Existing model and hosted teardown qualification gaps remain unchanged.
