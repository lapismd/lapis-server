# Governance

The project README documents how to run the image and set the initial owner
password. That documentation does not change SRV-SRV-001 or SRV-SRV-003.

## SRV-GOV-001 — Specification and source limits

<a id="srv-gov-001"></a>

**Requirement.** Authored specification Markdown MUST stay within 400 physical
lines and 32768 UTF-8 bytes. New production Rust and scripts MUST stay within
300 counted lines. Tests MUST stay within 500 counted lines. A protected change
MUST update its owning requirement, or record an unchanged-contract
confirmation with the validation and milestone review, in the same Jujutsu
change. Notes no longer owns this release gate. Server acceptance and the
image build run in this repository. Agents MUST run `pnpm spec:search` before
a manual scan of `spec/src`, use `--semantic` for a conceptual query, and use
`rg` only when QMD is unavailable. A git tag `v*` MUST publish
`ghcr.io/lapismd/lapis-server` through the image workflow, and this repository
MUST NOT publish an npm package.

### Acceptance details

- Spec-validator enforces the line and byte limits.
- CI installs mdbook 0.5.4 before the specification check, and Notes CI does not run server acceptance.
- GitHub Pages publishes Storybook at `/lapis-server/storybook/`, including these specification chapters, and does not publish a separate specification site.
- The server mounts published `@lapis-notes/workspace`, `main` requires a pull request plus the Server CI jobs, and a git tag publishes the image without npm.

## SRV-GOV-002 — Published shell styles

<a id="srv-gov-002"></a>

**Requirement.** Storybook MUST import the published `@lapismd/design-core/storybook.css` and `@lapismd/design-core/themes/lapis.css` styles before rendering `WorkspaceShell`. Shell stories MUST fill the viewport with no root padding. The preview MUST default the brand theme to `lapis` and expose the color-mode and theme toggles. The hosted application entry MUST keep loading the published design-core styles.

### Acceptance details

- The Storybook preview imports those published stylesheets and defaults `data-ui-theme` to lapis.
- The hosted Notes story requires grid layout, a loaded `--background` token, zero root padding, and full viewport height.
- The credential story renders the sign-in card inside the workspace shell with the same full-viewport contract.
- Each SUMMARY chapter has one Storybook Markdown mirror, and the docs table of contents follows those headings.
