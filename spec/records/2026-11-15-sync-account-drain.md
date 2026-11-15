# Native account close fencing and drain

15 November 2026. SRV-SYNC-019, enclosing Server change.
Status: source qualified; Desktop/OS integration open.

Ordinary commands remain synchronous single-flight admissions without a command
backlog. Explicit close now accepts at most one native drain, fences subsequent
commands and awaits the previously accepted action using an async semaphore.
Waiting for admission occupies no blocking worker. The accepted close task owns
its lifetime independently of its response waiter; actual custody/transport
continues off the async executor. Confirmed closure is idempotent.

The fence is released after an attempted close, including failed close or host
runtime cancellation before execution. A refused custody close retains the
original session/pending bytes for explicit identical-byte recovery; it never
selects another implementation. Runtime cancellation does not abort accepted
blocking effects or prove they completed. Hosts still retain the native owner
through failure and inspect an unknown outcome.

The exact regression failed 0/1 before: Close returned WouldBlock while a real
blocking custody action was admitted. The fixture always releases that worker
before asserting the failure so an old implementation cannot hang the test.
After the repair it passes 1/1. The strengthened family runs the close scenario
with one async executor thread and one blocking worker. It covers both a blocked
read followed by detached successful close, and a blocked failed write whose
drained close refuses, retains custody and permits explicit recovery. A second
close and subsequent ordinary work are refused while the drain is admitted.

Logs share .local/sync-account-drain-\*. Explicit RAM custody qualifies native
source behavior only. Real-server/cold-process, complete affected native tests,
strict Clippy and canonical checks follow before commit. No OS credential entry,
Tauri session, installed app, provider activation or publication is changed.
The full Sync roadmap and platform/device qualification remain open.

## Qualification attempts

The final Core owner family passes 5/5 with zero ignores
(`.local/sync-account-drain-family-final.log`). The real-server account family
passes 6/6, including its actual cold native child
(`.local/sync-account-drain-server.log`).

The first complete locked workspace run stopped at the unchanged `native_turn`
case `native_owner_acknowledges_durable_output_without_a_client_event_stream`:
10 passed/1 failed in that family. The expected Completed phase was Unknown,
with `output_coverage_lost`; its partial output was retained. Log:
`.local/sync-account-drain-workspace.log`. No assertion, skip, buffer, deadline
or concurrency was changed. The exact unchanged case then passed 1/1
(`capture-exact.log`) and its complete family passed 11/11
(`capture-family.log`), using the same log prefix.

That path uses the existing AI runtime's bounded 256-event journal and
100-millisecond capacity wait; it deliberately fails closed if native durable
publication cannot drain output in time. Sync account ownership is not invoked
by that test. Scheduling/storage contention is a hypothesis, not a proven
root cause. The failed attempt remains evidence; a complete unchanged locked
workspace recheck follows rather than treating an exact pass as full acceptance.

Before the initial rebuild, Cargo was stopped and 180 task-owned executable
artifacts identified from Cargo metadata were retired, reclaiming 13,274,991,984
bytes. Inventory: `.local/sync-account-drain-retired-tests.json`. No source,
fixture, global cache or unrelated target was removed.

Canonical checks pass validation, mdBook, spec-first and App ownership. Protocol
bindings match Rust and TypeScript checks pass. Doctor passes ten checks with
one existing script-alias warning, and QMD search for SRV-SYNC-019 succeeds.

The complete unchanged `cargo test --locked --workspace` recheck exits zero:
887 passed, zero failures, seven existing non-Sync ignores across 178 result
summaries (`.local/sync-account-drain-workspace-recheck.log`). All five current
Core owner scenarios are collected. The intermittent initial AI capture result
is retained above; its exact, full-family and complete-workspace rechecks pass.
No zero-test target or ignored test counts as functional acceptance.

Acceptance review: native close fences later admissions, drains accepted work
with one blocking worker, survives response detach, retains failed custody for
explicit recovery, and permits idempotent confirmed close. Desktop window/App
retention and actual OS custody remain separate incomplete consumer gates.

Final `cargo clippy --locked --workspace --all-targets -- -D warnings`
passes after the final test additions (`clippy-final.log`, same prefix).
The macOS test linker retains its existing large-unwind-table advisory; this
does not represent OS custody or packaged Tauri qualification.

Changed native files pass `rustfmt --edition 2024 --check` (owner and owner
tests). An additional exploratory `cargo fmt --all -- --check` reports existing
unformatted files outside this slice; the unchanged main version of Protocol's
`export-operations.rs` independently reproduces a format failure in
`.local/sync-account-drain-main-format-baseline.log`. No repository-wide format
pass is claimed and unrelated sources are not rewritten. Final canonical
validation/mdBook/spec-first/App-ownership and changed Markdown formatting pass.
