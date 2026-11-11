# Agent CLI version tags and configuration

## Before and intended behavior

Anonymous GHCR probes of the current 4a40488 release confirm all four
`<agent>-<cli-version>-4a40488` indexes are public for AMD64 and ARM64.
The shorter requested CLI-version aliases return 404 for Codex 0.167.0,
Cursor 2026.11.05-86047df, Claude 2.1.311 and OpenCode 2.2.6: four missing aliases.
Local before evidence is `.local/mod-version-tags-before.json`.

The existing release step already assembles one multiarchitecture index from
qualified native artifacts. Add `<agent>-<cli-version>` to that same
`imagetools create` invocation while retaining both existing tag forms. The
catalog remains the single version source; payloads are not rebuilt for aliases.
Version-only aliases may move when packaging changes. A digest selects exact
bytes; a version-plus-commit tag selects the source build.

The root README lists the current pinned CLI versions and shows how to choose
agents through pipe-separated `DOCKER_MODS`, start the dedicated Compose example,
retain a separate verified cache, configure strict/debug/offline behavior and
supply daemon-host config/secret paths. Root/writable initialization and the
UID/GID 1993 application boundary are explicit. Agent credentials remain separate.

## Validation and publication gate

Workflow actionlint, Compose interpolation, formatting, specification and demo
script checks qualify the workflow/documentation change. Their results are
recorded below. Actionlint passes; all eight demo-script checks pass; the README Compose
selection interpolates the expected image, pipe-separated agents, UID 0,
writable layer and independent cache on context omarchy. Two spec attempts rejected a multi-sentence bullet and then excess acceptance
bullets; the final four atomic statements preserve the contract and validator
limits. The final spec check (including App-ownership audit) and repository formatting
check both pass.
Existing native AMD64/ARM64 artifact qualification
and version commands remain unchanged and precede publication.

After merge, confirm all four CLI-version aliases are anonymously public and
have exactly the same digest/platforms as their corresponding version-plus-commit
and commit tags. Four aliases returning 404 is the before count; local static
checks cannot establish the after publication result. No npm publication is
part of this change; Terminal UI remains held.
