# Packaged CLI skill references

The CLI entrypoint now routes to three focused references: commands and outcomes,
exact host/credential selection, and protected skill management. These document
the actual shared CLI and operation contracts, including offline help versus
live permission checks, skills:read versus skills:write, pagination, exit codes,
pending receipts and skipped updates. No Obsidian-only command is advertised.

The native host contribution explicitly embeds all three resources alongside
SKILL.md. `cargo test --locked -p lapis-app-core --test packaged_skills
cli_skill_installs_its_linked_reference_resources` failed 1/1 before the change
because the command reference was absent, then passed 1/1. It installs every
reference into an isolated vault, compares bytes and verifies that editing a
reference skips the whole skill on update without replacing SKILL.md.

Consumer intake and host releases are separate qualification steps.

The complete Core/Protocol/CLI Rust suite passes 652 cases, with seven existing
native-model qualification cases ignored by their unchanged opt-in requirements.
Strict all-target Clippy passes with -D warnings. Canonical spec/first/build and
the App-ownership audit pass after updating the mapped native/governance chapters.
`lapis --version`, `lapis help skills install`, `lapis help -k skills` and
`lapis note list --help` match the documented version, operation fields, offline
keyword guides, pagination and host/credential flags. All four skill entrypoints
pass the skill-creator frontmatter validator across the owning repositories.
