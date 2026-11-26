# Absolute job DNS regression

SRV-JOB-009 requires asynchronous DNS without search-suffix widening. The
broker appended a dot to already-qualified names, producing an invalid double
dot. The shared private normalizer now preserves an existing terminal dot.
The resolver cancellation test uses the production normalizer for both forms.

On Omarchy source `2a84340dc35696334b520d19d8d9f22d5ecb1762`, the exact
`cargo test --locked -p lapis-app-core --lib stalled_dns_is_cancelled_without_blocking_runtime_shutdown`
failed: zero passed, one failed. The failure was the already-qualified input.

After correction, source `c7d4422e7cda00ab9b2a32bdfc6bc54147913526` passed:

- The same exact resolver test: one passed, zero failed.
- `cargo test --locked -p lapis-app-core --test service_background_network`:
  nine passed, zero failed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --locked --workspace`: 1,029 passed, zero failed, twelve existing
  ignored cases across 206 summaries.

Container `lapis-plugin-jobs-network-fqdn-after-20261126` exited zero. Its final
structured receipt confirms every command passed. The local attach collector
ended early; the complete Docker log was collected separately rather than
rerunning successful tests. Logs and failed attempts remain in ignored local
receipts and the remote cache. Hosted synthetic-bank acceptance remains pending;
this source result does not qualify actual provider or host custody behavior.

The first documentation check rejected a two-sentence acceptance bullet; it was
rewritten as one atomic bullet before retrying the same checks.

The next documentation check required the mapped governance and server chapters;
those cross-references were added before the final retry.

The mapped governance chapter exceeded its file budget; existing fixture prose
was condensed without removing its assertions and the DNS link was shortened.
