# Governance

## SRV-GOV-001 — Specification and source limits

<a id="srv-gov-001"></a>

**Requirement.** Authored specification Markdown MUST stay within 400 physical
lines and 32768 UTF-8 bytes. New production Rust and scripts MUST stay within
300 counted lines. Tests MUST stay within 500 counted lines. A protected change
MUST update its owning requirement, or record an unchanged-contract
confirmation with the validation and milestone review, in the same Jujutsu
change. Notes no longer owns this release gate. Server acceptance and the
image build run in this repository.

### Acceptance details

- Spec-validator enforces the line and byte limits.
- CI installs mdbook 0.5.4 before the specification check, and Notes CI does not run server acceptance.
- GitHub Pages publishes this specification at `/lapis-server/` and Storybook at `/lapis-server/storybook/` from registry packages using Node 24.
- The server mounts published `@lapis-notes/workspace`, and `main` requires a pull request plus the Server CI jobs `Specification` and `Configure the server package`.
