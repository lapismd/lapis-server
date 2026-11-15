# Lapis Sync encrypted object repository — 2026-11-15

This is the first clean Server-owned native Sync implementation slice. It is
based on Server main `2dd27c10` and the canonical contract change `f76a5d29` in
the independent repository `/tmp/lapis-sync-implementation.Vk9Fyp/lapis-server`.
Sync remains optional and disabled; no HTTP route, enrolled participant,
working-vault scheduler or application consumer selects this repository.

## Rejected draft baseline

The preserved local bookmark `sync/unsafe-stage-7-draft` contains the earlier
Stage 3 through Stage 7 draft without canonical requirements. Its exact
destination target:

```sh
cargo +1.99.0 test -p lapis-server --lib control::destination -- --nocapture
```

failed **0/6** cases. The wider Server library run passed **27/33** and failed
the same six destination cases. The draft also wrote `uploads/` and `objects/`
relative to the process working directory, ignored enrolled-device authority on
reads, exposed no destination HTTP route, exceeded production/test source
limits and used SQL state encodings that violated its own checks. It remains
isolated provenance and is not a base for this slice.

## Implemented boundary

`lapis-app-core::sync::ObjectRepository` binds one repository to an admitted
absolute profile, logical vault and destination identity. It creates only
private versioned directories under that profile and retains the physical
profile identity. Replacement, symlinked components, invalid identities and
non-directory components fail closed.

Encrypted object identity is the lowercase SHA-256 digest of its complete
ciphertext. Publication streams through a 256 KiB buffer into a private staged
file, verifies the digest, synchronizes the bytes, publishes without replacing
an existing object and synchronizes the containing directory. A lost
acknowledgement retry verifies and reuses the durable object. Mismatched or
corrupt existing bytes are neither replaced nor served.

The reader validates the complete stored ciphertext before emitting bytes, then
rehashes the bounded stream so concurrent mutation remains an explicit error.
Known corrupt objects leave the caller's target untouched. Event, attestation,
membership, HEAD, outbox, apply-journal and aggregate transfer-budget ownership
remain later slices.

## Verification

The exact test initially did not exist. After implementation:

```sh
cargo +1.99.0 test -p lapis-app-core \
  --test sync_object_repository --locked -- --nocapture
```

passes **7/7** cases covering stable retry reuse, mismatch cleanup, corruption
refusal, independent profile/vault/destination scope, replaced-profile refusal,
repository and object symlinks, and bounded streaming.

Strict affected Clippy passes:

```sh
cargo +1.99.0 clippy -p lapis-app-core \
  --test sync_object_repository --locked -- -D warnings
```

The complete Core suite passes **580 tests** with **7 existing explicit
asset/helper ignores** across **587 collected tests**. The ignored cases are the
pre-existing ONNX/model and isolated crash-process lanes; none belongs to Sync.
`pnpm spec:check` passes validation, mdBook, spec-first and the App-ownership
audit. The new production module is below 300 lines and its test is below 500.

The first full-workspace attempt reached Sync at 7/7, then one unrelated
`http_files` case failed at 53/54 with a loopback connection reset. Its exact
rerun passed 1/1, the complete binary passed 54/54, and the final locked
full-workspace run exited successfully. This evidence does not reclassify that
transient as a Sync failure or hide the preserved first result.

No Docker, Linux image, HTTP, real server process, Tauri, OS credential,
independent-device or installed-application boundary is claimed. The Changeset
records future Server release intent only; publication, tagging, image delivery
and deployment remain unauthorized release gates.
