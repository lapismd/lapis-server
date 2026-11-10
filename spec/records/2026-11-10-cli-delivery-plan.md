# CLI delivery requirements

The content automation backlog now explicitly requires Changesets-driven CLI
versioning, Markdown-first help useful to agents and separate CLI downloads on
GitHub releases. This is a plan refinement, not implementation or qualification
of new commands, release assets or supported platforms.

## Source review and decisions

- The existing `lapis-cli` Cargo version is `0.1.0`; Clap supplies `--version`.
  The server's Changesets release version is separate today, and release tooling
  does not build/upload CLI archives. The delivery slice must remove that drift
  using one Changesets release version, without tying protocol compatibility to
  binary SemVer or changing the other private crates' versions unnecessarily.
- The custom Rust `jj` CLI in `jj-store` embeds authored Markdown help, keyword
  guides and build information, including JSON version output. It supplies the
  reference conventions; no source is copied and the CLI gains no jj-store
  dependency. Server QMD search passed; reference-repository QMD reported a Node
  ABI mismatch, so its permitted `rg` fallback located JST-CLI-002 and its help
  sources. No global tooling or sibling source was changed.
- Agent guidance must document discovery, scope, preconditions, structured
  results and recovery without blind replay. Existing raw note-byte and JSON/MCP
  output contracts remain distinct from Markdown help and human summaries.
- Separate, checksummed, versioned CLI archives must run without Desktop or an
  image; actual extracted-target acceptance gates advertised support. This
  documentation adds no distribution workflow and publishes no artifacts.

## Changesets audit

Changesets is enabled for the private server package, including version/tag
generation and the custom GitHub/image release script, with npm publication
disabled. Release workflow `42723373052` succeeded after the Pages fix merged.
Open [version PR #15](https://github.com/lapismd/lapis-server/pull/15), head
`e236d5518fc8d7a213f4420ea31b7fe795745f70`, proposes `1.0.2` and consumes all
17 pending Changesets, including hosted content settings and Pages recovery.
Its initial CI run `42723459522` was `action_required`: GitHub required approval
for the bot-created PR. That run was approved after inspecting the version-only
diff; its checks and merge remain separate from this documentation validation.

## Contract and acceptance review

SRV-SRV-010/012 and SRV-GOV-001 retain their implemented behavior and evidence.
CLI delivery remains pending in its owning backlog; no existing source or host
receipt is promoted to release qualification. Validation is formatting and
`pnpm spec:check`, with spec-validator doctor and QMD discovery. Product suites
are out of scope for this documentation-only change.
The initial specification check passed validation/build but rejected the missing
canonical unchanged-contract confirmation; adding it to governance resolved that
failure without changing the implemented CLI or release behavior.
