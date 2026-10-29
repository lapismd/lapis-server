# Native vault source provenance

The seven vault engine modules were moved from `lapis-desktop`
`src-tauri/src/vault` at qualified source revision `40a5d97c`, under Apache-2.0:
`action_targets.rs`, `conditional.rs`, `creates.rs`, `files.rs`, `mutations.rs`,
`staged.rs` and `writes.rs`.

The extraction removes those implementations from Desktop and makes it a direct
consumer. Public native witness/accessor methods replace crate-private access;
file policy, bounds and publication semantics are retained. Host profiles,
resource admission and OS integration remain Desktop-owned.
