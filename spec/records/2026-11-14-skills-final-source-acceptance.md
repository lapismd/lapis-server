# Final skill source and public-package acceptance

`cargo test -p lapis-app-core --locked` passes all 580 tests with seven existing
ignored cases. This includes the final enabled/revoked-source rules and protected
skill installation suite. No native test or diagnostic is weakened.

The final API development archive is consumed through the documented ignored
pnpm hook. Its SHA-256 is `3616fa66ac635b2cbe6bddac2c2b3af911781989fe411b42b39ba245a86e9f1e`.
The normal registry manifest and lockfile are restored afterward. The complete
web suite passes 131 cases; strict source types and App ownership pass with zero
warnings. The native host, HTTP/MCP and actual CLI acceptance remain qualified
as recorded in [the feature receipt](2026-11-14-plugin-agent-skills.md).

The final API source also supplies real browser conditional writes, vault-scoped
Web Locks and cache-safe projection. Its own 1,803 tests and public-export Chromium
acceptance cover those portable concerns, including a cold browser process.
Server/CLI execution retains the native Core writer and authenticated grant scope.

Qualified Server code: `aee3c957`; API feature change: `uyksuztq`.
Publication, released package intake and native application distribution remain
separate gates. No temporary file dependency or source override is committed.

A concurrent committed release refresh changed only package/changelog metadata
in the feature ancestry. Runtime-tree parity was checked against `aee3c957`;
the refreshed feature tip is `8aa7c871`. Its 131 web tests, source checks and
specification gates were rerun. The first documentation-only spec-first attempt
required its owning governance link; the final linked receipt passes.
