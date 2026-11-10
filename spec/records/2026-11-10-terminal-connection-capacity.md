# Terminal HTTP connection capacity

Widened hosted acceptance exposed six failures after default Terminal intake.
A diagnostic trace showed two browser tabs holding six long-lived HTTP/1
connections (Terminal output plus database observation). Editor admission
could not acquire a connection, so the command palette remained waiting for
file open. The same exact switch/refusal test passed without Terminal intake.

Output responses now end on an idle read, after 16 frames, or after a
one-second lease. The renderer resumes from the byte cursor and yields
connection capacity between responses. The shell itself remains owned by its
vault client. Authentication is still revalidated before and after each read.

The original multi-browser Tasks switch/refusal failure passes after repair.
The wider run improves from six failures to one, with 14 of 15 tests passing;
the remaining startup Retry failure is separately isolated to default AI
cleanup of an unselected native turns service, not connection capacity.

The finite-response client regression fails before repair (one request instead
of two), then passes with seven Terminal client cases. All four native Terminal
HTTP tests pass, including a live-shell EOF regression. The owning Server Rust
package passes all 132 tests and all-target Clippy with warnings denied.
The earlier whole-workspace result remains 774 passes and seven existing
asset-gated ignores; unchanged Core/protocol behavior is covered by that run.
