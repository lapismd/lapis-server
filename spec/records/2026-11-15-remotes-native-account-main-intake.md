# Integrate concurrent native account transport

The repository-remotes feature integrates Server main
`17bb15ea61434ee74447bb5044a841800141c69b`, including the separately owned
Lapis Sync account transport. Git backup synchronization remains independent.
The integration retains the combined jj-store source
`086def182eae2ab6e5dc39aba030390d7ea8612c` and all existing remote custody,
publication and recovery policy.

Hosted CI run `43754330219` initially refuses a 402-line governance chapter
in the newer merged context. Remote qualification prose moves intact to the
existing nested governance topic with corrected relative record links. The
root chapter is now 377 lines; this does not weaken the chapter limit.

The first native workspace command fails with one duplicate TOML key: both
branches added identical `zeroize = "=1.9.1"` declarations. Removing only the
duplicate preserves the pinned dependency. The same
`cargo test --locked --workspace` command then passes 916 tests, zero failures
and 11 explicit opt-in ignores across 184 result summaries. Strict all-target
workspace Clippy and the workspace build pass on source snapshot
`4ede2c8d5c250ae83178296dcc3794b1dc66ac28` through the isolated pinned Linux
validation image recorded in the preceding native receipt.

Specification, App ownership, format and manifest checks remain required before
the integration push. These native checks do not qualify Desktop, final registry
browser adoption, live HTTPS provider authentication or cold application release.
