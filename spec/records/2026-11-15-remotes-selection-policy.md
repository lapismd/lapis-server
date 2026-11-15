# Private target-aware repository connection selection

Change `yqmnvzxx` extends shared Core policy under SRV-SRV-004 and the
credential separation boundary in SRV-SRV-016. This is a source foundation;
transport management, admitted host routes, scheduler adoption and cold
Desktop/hosted acceptance remain outstanding.

Core stores the two targets in installation-private storage outside the vault.
Records contain validated endpoints and opaque credential references, never
secret bytes or native host paths. Revision updates use a process lock and
atomic conditional write. Pending-publication inspection runs while that lock
is held; an unavailable inspection refuses the save without changing bytes.
New/changed backup destinations start in manual mode. Original changes preserve
the backup preference. A one-time legacy import retains an explicitly saved
preference, while anonymous connections cannot enable automatic publication.

Baseline `node .local/remotes-validation.mjs test --locked -p lapis-app-core
--test repository_remotes` reported the absent integration target. The first
four source cases passed. Changing the pending check from a boolean to a
callback exposed two test compilation errors (E0277 and E0308); the corrected
exact target passed four cases. The final six-case target adds lock contention,
failed pending inspection and anonymous legacy preference coverage; all pass.

Validation uses Rust 1.99.0, Node 24.10.0 and pnpm 10.34.5. An ignored helper
exports the tracked JJ snapshot into an isolated Linux Docker worker with
registry/git/target caches, two CPUs and 8 GiB; it transfers no private profile,
secret or ignored workspace files. This replaces local compilation while local
disk remains below the native-library reserve, without changing that reserve.

- `node .local/remotes-validation.mjs test --locked -p lapis-app-core
-p lapis-app-protocol`: 628 passed, zero failed, seven existing opt-in ignores
  across 124 suites. This includes the six selection cases.
- The exact six-case target passed independently, zero failures.
- `node .local/remotes-validation.mjs clippy --locked -p lapis-app-core
-p lapis-app-protocol --all-targets -- -D warnings`: pass.
- `pnpm spec:check`: canonical validation, mdBook, mapped spec-first and App
  ownership audit passed.
- `pnpm exec spec-validator doctor`: ten passes and one existing script-alias
  warning. QMD opened and `pnpm spec:search "remote selection"` returned the
  owning source. An initial literal `--` query was refused; positional syntax
  corrected it.

Unrelated formatter output was identified by comparing each file against the
formatter output of its parent revision. All 35 exact mechanical matches were
restored before committing; no semantic work was removed.

A final helper audit found that concurrent invocations reused their source
archive staging path. The helper now gives every invocation a unique directory
and prints its source revision/image identity. The isolated snapshot
`28e3eea211688660960f10eca6648a5a864b3b43` passes all 628 cases again (zero
failures, seven opt-in ignores, 124 suites). Strict all-target Clippy also passes
from its own isolated archive. The unchanged source files, rather than the
earlier shared-staging label, qualify this receipt.

No native jj-store dependency revision changed in this slice. SSH transport
intake, actual credential resolution, private/original Git configuration
coordination and host acceptance remain separate qualification gates. No
application release or public crate publication is claimed.
