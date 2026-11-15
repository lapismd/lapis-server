# Lapis Sync account consent — 2026-11-15

This fifth clean Server slice extends the rebased arrival stack at `7e0d14e8`.
It establishes durable account-connection consent in the existing private
control store without exposing a route or granting endpoint/content authority.

## Exact boundary

The new target is:

```sh
cargo +1.99.0 test -p lapis-server \
  --test sync_account_connections --locked -- --nocapture
```

Its first compile found that the new lifetime constants were not exported from
the control module. After repairing that public module boundary, the exact
target passes **3/3**.

The first widened `lapis-server` package run stopped while linking because the
isolated task volume had only 1.1 GiB free and the generated Rust target had
grown to 41.9 GiB. Removing only that task-owned build output restored 43 GiB;
the same package boundary then passed **151/151**. This was an environment
capacity failure, not a product-test failure.

## Implemented boundary

A native client creates a ten-minute request bound to its stable client UUID,
bounded display name and SHA-256 S256 challenge. The verifier is never stored.
Only an active, unexpired, unrevoked owner session can approve or reject the
request. A rejected, expired or already consumed request cannot issue a
credential.

Successful redemption uses the exact verifier, consumes the request once and
atomically creates a 30-day discovery credential. Only the bearer's SHA-256
hash is stored. The bearer can resolve its client identity after a cold reopen and can
be revoked without deleting history. Account connection still grants neither
endpoint enrollment nor encrypted content, membership, owner or recovery
authority.

The exact cases cover real owner login, correct/wrong verifier, nonexistent
owner session, expiry, rejection, replay, hashed storage, cold reopen,
revocation, nil client identity and malformed challenge/name bounds. A wrong
verifier attempt is also proven non-consuming. The production module is below
300 lines and the exact test is below 500.

## Verification

- Targeted Rust formatting for the two new source/test files: passed.
- Exact account target: **3 passed, 0 failed**.
- Existing authentication plus account cohort: **7 passed, 0 failed**.
- Full `lapis-server` package: **151 passed, 0 failed, 0 ignored**.
- Strict workspace/all-target Clippy with warnings denied: passed.
- Canonical specification, mdBook, spec-first and App-ownership lanes: passed.
- Svelte/type check: **0 errors, 0 warnings**; CLI distribution: **4/4**;
  Web unit suite: **132/132**; production Web build: passed.
- QMD semantic search degraded to its documented `rg` fallback because the
  system QMD native module does not match the active Node 26 process ABI 147.
  `spec-validator doctor` otherwise passed repository
  configuration, summary, book, ignore, QMD configuration/binary and skill
  checks; the repository checker itself passed without findings.

## Unclaimed boundaries

No HTTP/browser handoff, vault discovery response, Desktop credential custody,
sign-out integration, pairing, content key or server participant is exposed.
Those require separately qualified host routes and consumers. Source evidence
does not qualify Docker images, Tauri, installed applications or activation.
