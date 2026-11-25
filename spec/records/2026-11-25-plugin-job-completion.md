# Plugin job terminal completion

This record covers SRV-JOB-001 and the shared headless execution boundary.
A handler could catch the last native callback error and return after its
execution deadline or cancellation. Completion now checks the retained host and
deadline after JavaScript settles. An already-admitted native mutation still
settles before its execution lease ends; no retry or rollback is introduced.

All executable qualification ran in Docker context `omarchy`.

- The two new exact regressions failed before the source change (2 failures).
- The same two regressions passed after the source change (0 failures).
- `cargo test --locked -p lapis-plugin-jobs`: execution 10 and scheduling 5
  passed. Core background-job acceptance passed 10 tests.
- Strict workspace Clippy passed in the same source cohort.
- The first full Rust workspace run exposed one `WouldBlock` on reopening the
  existing `turn_journal` fixture. Its exact
  `duplicate_and_reused_requests_never_create_a_second_dispatch_claim` test,
  complete owning suite and fresh `cargo test --workspace --locked` all passed
  without changing that fixture, its assertions or product journal code.

The full successful retry used source
`76e00ef795e744c03a461d51997aca42eb2077e4`, pack SHA256
`7a61220e5707b8b6427000f5269e4441a6a2d9246dba7066e5b6a6a220fea504`,
and container `lapis-plugin-jobs-deadline-full-retry-20261125`.
Local logs retain the before, first after, and full retry outputs under
`.local/plugin-jobs-deadline-{before,after,full-retry}.log`.
Registry adoption and host qualification remain independent gates.
