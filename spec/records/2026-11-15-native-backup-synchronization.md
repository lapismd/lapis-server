# Native vault backup synchronization

This source slice implements the existing explicit automatic-sync selection in
Core. The renderer does not run a transfer scheduler. Original repositories
remain manual and saving a connection performs no transfer. Browser/desktop UI,
real HTTPS/forge sign-in and application publication remain separate gates.

## Baseline and behavior

The exact new admitted-selection regression is
`cargo test --locked -p lapis-app-core --test repository_remotes_runtime disabling_automatic_sync_is_a_guarded_native_selection_without_network_or_capture -- --exact --nocapture`.
It fails once with `Unsupported` before the implementation and passes afterward;
all six remotes runtime cases pass. An initial abbreviated name combined with
`--exact` selected zero cases and is not qualification evidence.

A native scheduler belongs to the admitted vault and retains only weak vault
ownership while idle. Each cycle reserves ordinary accepted work and the shared
backup transition turn. Private selection, installation custody and process
fences remain shared with manual operations. Explicit enablement requires host
transfer capability, admitted credentials and enabled local backup; disabling
remains possible without credentials or capture initialization.

The existing native engine captures, imports and prepares the merge. Conflicts
remain retained proposals and use native inspection/resolution. Before physical
application, Core saves the exact guarded restore identity in private host state.
An uncertain apply pauses after restart until explicit exact-request recovery or
keep-current. Native ancestry and dirty-editor guards remain authoritative.
Publication pins one native intent in the existing backup journal and reconciles
uncertain publication before permitting another receive.

Passive status reads use an in-memory native observation and never wait behind
network turns. Explicit recovery controls use the ordinary accepted-work turn.
Successful close drains an accepted cycle; refused close keeps the same live
owner. No idle work lease prevents closure. Neither status nor private journals
contain raw credentials or browser-selected storage paths.

## Actual native transport and HTTP evidence

The first complete new actual-SSH case passes. A stronger execution additionally
waits for the real blocked network observation and proves successful close waits
for it before draining. This is native transport evidence, independent of UI
fixtures. `scripts/test-repository-remotes-ssh.mjs --sync-only` qualifies:

- Saving remains manual; idle work count is zero.
- Passive status remains observable during a held SSH operation, including after
  the initiating client detaches.
- Two independent vaults converge saved changes through native merging while
  preserving separate local work and all unrelated remote branches.
- Conflicts preserve physical files until an explicit native resolution.
- Dirty-editor apply refusal retains the original request; refused close keeps
  that owner, cold reopening does not replay it, and explicit exact recovery
  applies the reviewed tree.
- Lost push responses retain the original publication; restart reconciles it
  without another receive.
- Successful close joins a held accepted cycle and preserves its exact result.
- A saved manual preference survives another cold restart without publication.

The authenticated HTTP regression initially expects 422 for an unknown query
field; the existing Axum query boundary correctly refuses it with 400. The test
is corrected to that exact query contract and passes. JSON command injection
still requires 422 and oversize JSON requires 413. Owner, generation, vault,
origin, protocol, no-store and retired-client checks pass. Cookie/CSRF checks
cover both ordinary remotes and synchronization in the complete workspace lane.

## Complete qualification

Checked native source snapshot: `4748571debc7812a661e8e316812a3035a4127ab`.
Execution uses the existing isolated Linux validation lane, pinned image
`sha256:6031c86d7c26d9e7011c958374462c600b10aa6a586ad7196d741756df0a5a32`,
Rust 1.99 and the unchanged portable jj-store revision
`6649b7e95ee58823ba28f0a9ca01e507b458edd9`.

| Command / boundary                                                | Result                                                                          |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| Exact opt-out regression, complete remotes runtime family         | 1 baseline failure; 6 final passes                                              |
| New actual SSH synchronization case                               | 1 pass, both initial and strengthened runs                                      |
| Exact passive HTTP admission case                                 | 1 initial assertion mismatch; 1 final pass                                      |
| `cargo test --locked --workspace`                                 | 871 passes, zero failures; 11 explicit opt-in ignores across 176 suites         |
| `cargo clippy --locked --workspace --all-targets -- -D warnings`  | Pass                                                                            |
| `cargo build --locked --workspace`                                | Pass                                                                            |
| `node scripts/test-repository-remotes-ssh.mjs`                    | 4 actual SSH cases pass: original, backup, synchronization and Server transport |
| Native Protocol exporter / generated bindings / public TypeScript | Generated from Rust; both bindings refreshed; TypeScript passes                 |
| `pnpm spec:check` / App ownership                                 | Pass; latest Deno-free validator 0.2.0                                          |
| Doctor / QMD semantic discovery                                   | 10 checks pass, one script-alias warning; search runs                           |

The first native compile reports two visibility/guard-coercion errors; both are
corrected before the passing native runtime, transport and workspace gates.
Local all-workspace rustfmt also touches unrelated pre-existing formatting;
those incidental paths are restored, and changed production/test files retain
their scoped formatting. No dependency pin, assertion, timeout, visual baseline
or qualification rule is weakened.

Generated bindings come from the native Rust `export` binary and the same public
Prettier formatting used by `scripts/protocol.mjs`; the cross-platform validation
helper and cache locations remain ignored. Production code contains no fixture
imports or acceptance switches. No package or image is published by this slice.

## Current-main integration

The first hosted feature run (`43745478778`, source `6ce02445`) stops at the
canonical chapter-size gate: the merged Server and governance chapters exceed
400 lines after concurrent main additions. Integrating main `a32ab23d` preserves
its Design Core 0.13.4 intake and independent Lapis Sync work. Detailed remote
requirements move to the linked repository-remotes chapter; qualification prose
moves to the existing remote governance page. Requirement IDs remain anchored in
Server. The nested qualification links are corrected after the first split
check identifies two broken relative paths.

Merged native source `6f5e7006b246c2e88f49fb2f02d9b464851057db` passes all 905
workspace tests with zero failures and the same 11 explicit opt-in ignores
across 182 suites, strict workspace Clippy and the workspace build. Final spec
and App-ownership checks pass. No runtime behavior is changed by the chapter
split. Hosted feature CI remains a separate gate.
