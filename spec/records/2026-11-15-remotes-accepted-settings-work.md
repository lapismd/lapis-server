# Accepted native repository settings work

Change `vqkzqxrp` establishes the shared Core work-lifetime boundary under
SRV-SRV-004/016. It adds an installation-selected RemoteStorage and a passive,
secret-free credential inventory port. Host DTOs select neither storage paths
nor providers. The existing backup accepted-turn queue serializes private
connection saves; native work owns its lease independently of the response.

The read/save foundation exposes private backup connection selection with manual
defaults. Source snapshots overwrite provider-supplied target/sync fields with
native selection. They advertise no fetch or push actions. Original mutation,
network operations, actual credential writes, native scheduling and host route
adoption remain unimplemented in this service; existing application profiles do
not select it. It is not a completed remote manager or application slice.

The exact baseline
`node .local/remotes-validation.mjs test --locked -p lapis-app-core --test
repository_remotes_runtime` reported the absent test target. The first new
fixture compile reported one E0624 error because it called a crate-private error
constructor. Using the public IO conversion corrected that fixture. Three
cases passed, with one unused fixture import warning; removing the import then
produced the final warning-free exact pass. Assertions were preserved.

The cases use the real Core runtime and accepted-work queue with a controllable
passive inventory provider. They prove manual/private save without native
repository initialization; stale-write refusal; detached response fencing with
successful observation by another client; and cancelled-response ownership
through vault close, drain and reopening of persisted selection. They do not
qualify actual credential custody or network transport.

Validation uses immutable, revision-labelled Linux source archives and the
existing ignored remote helper, Rust 1.99.0, Node 24.10.0 and pnpm 10.34.5:

- Exact runtime target: 3 passed, zero failed or ignored.
- Full Core/Protocol/Server tests: 795 passed, zero failed, seven existing opt-in
  ignores across 154 suites.
- All-target strict Clippy for all three crates: pass.
- Native Server build: pass.
- `pnpm spec:check`: canonical validation, mdBook, spec-first and App ownership
  pass. The unchanged JS lockfile is verified before reusing the sibling
  workspace's installed registry dependency directory through an ignored link;
  no package manifest, source alias or package override is involved.

One orchestration attempt refused to snapshot a stale workspace before running
Cargo. Refreshing the parent intake produced a governance append conflict and
two variants differing only in the unused test import. Both governance topics
were retained, the corrected import selected, and the redundant variant retired
with recoverable Jujutsu history. No concurrent external work was changed.

Next: extend this accepted worker with admitted native remote operations and
review/execute/observe identities, then wire encrypted/OS custody and both host
adapters. Independent cold native/hosted acceptance and releases remain open.
