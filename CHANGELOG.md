# @lapismd/lapis-server

## 1.0.2

### Patch Changes

- [#16](https://github.com/lapismd/lapis-server/pull/16) [`fd92fb5`](https://github.com/lapismd/lapis-server/commit/fd92fb521f9ac49a20bb97293c32f50c5e31cbaa) Thanks [@stevejuma](https://github.com/stevejuma)! - Extend the opt-in debug backup diagnostics across shutdown and final capture
  with fixed safe categories while preserving public errors and backup behavior.

- [#28](https://github.com/lapismd/lapis-server/pull/28) [`7dadbfc`](https://github.com/lapismd/lapis-server/commit/7dadbfcf7859fd639cdbf1526f8879c759a36b2f) Thanks [@stevejuma](https://github.com/stevejuma)! - Add shared native content-access controls for host settings: disabled startup,
  named read-only credentials, scope/expiry status, disable/re-enable with fenced
  drain and vault retirement. Production endpoints and plugin screens remain pending.

- [#24](https://github.com/lapismd/lapis-server/pull/24) [`19727d3`](https://github.com/lapismd/lapis-server/commit/19727d3f539142f3dcd643899973138ffeceea48) Thanks [@stevejuma](https://github.com/stevejuma)! - Add an initial lapis CLI client for capabilities, admitted vault/note lists and
  note reads, with protected credential input, JSON output and stable exit codes.
  Production host mounting, writes, MCP and CLI distribution remain pending.

- [#27](https://github.com/lapismd/lapis-server/pull/27) [`c8345f0`](https://github.com/lapismd/lapis-server/commit/c8345f00bdbcbe0b0c9408ae2188b8ca0d68db64) Thanks [@stevejuma](https://github.com/stevejuma)! - Add a source CLI MCP stdio bridge to the shared host endpoint, with protected
  credential files, current and legacy stdio negotiation, bounded requests and
  safe shutdown. Production host mounts and plugin settings remain pending.

- [#19](https://github.com/lapismd/lapis-server/pull/19) [`aa2940c`](https://github.com/lapismd/lapis-server/commit/aa2940c2811fbf59d530ac5116cb04a16444ce49) Thanks [@stevejuma](https://github.com/stevejuma)! - Add portable content automation contracts for shared Server, Desktop, CLI and
  MCP integration, with strict input validation and generated protocol drift
  checks. This foundation does not enable new endpoints or external access.

- [#22](https://github.com/lapismd/lapis-server/pull/22) [`b9ac7ae`](https://github.com/lapismd/lapis-server/commit/b9ac7ae685d1bf3509ddc1d1653bbfd32635b603) Thanks [@stevejuma](https://github.com/stevejuma)! - Add a shared read-only content HTTP adapter with scoped credentials, strict
  origin/input checks, bounded native admission and direct UTF-8 note responses.
  Production Server/Desktop mounting and CLI/MCP integration remain pending.

- [#26](https://github.com/lapismd/lapis-server/pull/26) [`b1260ea`](https://github.com/lapismd/lapis-server/commit/b1260eac2e570dd00f817ef2c44c7c449a960db4) Thanks [@stevejuma](https://github.com/stevejuma)! - Add a shared read-only MCP adapter with official SDK interoperability, scoped
  tools and note resources, and shared HTTP admission for Server/Desktop hosts.
  Production mounting, OAuth, writes and the CLI stdio bridge remain pending.

- [#20](https://github.com/lapismd/lapis-server/pull/20) [`afdbe8c`](https://github.com/lapismd/lapis-server/commit/afdbe8cee7930c475806f478ae378096d9c233bc) Thanks [@stevejuma](https://github.com/stevejuma)! - Add shared read-only native content sessions with vault admission, revocation,
  expiry, bounded reads and snapshot-bound pagination. External endpoints and
  content writes remain disabled pending their own integration slices.

- [#22](https://github.com/lapismd/lapis-server/pull/22) [`1836958`](https://github.com/lapismd/lapis-server/commit/1836958fe39dffb237ad7a3f0378c3da45ae4028) Thanks [@stevejuma](https://github.com/stevejuma)! - Add shared ephemeral read-only content test credentials with bounded admission,
  redacted secrets, scoped dispatch, expiry cleanup and revocation/shutdown drain.
  Host endpoints, persistent grants and external consent remain pending.

- [#31](https://github.com/lapismd/lapis-server/pull/31) [`38910a1`](https://github.com/lapismd/lapis-server/commit/38910a1e505a7d6bb02457ec06a8b04a4830bd2d) Thanks [@stevejuma](https://github.com/stevejuma)! - Add optional read-only content API and MCP endpoints to the hosted server, with plugin settings for scoped expiring credentials, revocation and disablement. Persist the enable preference while keeping credentials process-local and out of browser settings storage.

- [#25](https://github.com/lapismd/lapis-server/pull/25) [`9ecdcbc`](https://github.com/lapismd/lapis-server/commit/9ecdcbca6be91b30cfa749adb8a8ff15c4840dc0) Thanks [@lapismd-dependency-bot](https://github.com/apps/lapismd-dependency-bot)! - Refresh first-party Lapis dependencies to the published versions recorded in this pull request.

- [#13](https://github.com/lapismd/lapis-server/pull/13) [`c8dd72a`](https://github.com/lapismd/lapis-server/commit/c8dd72a302edfd100b1be03669ec49ff60ba698e) Thanks [@lapismd-dependency-bot](https://github.com/apps/lapismd-dependency-bot)! - Adopt Design Core 0.12.2 and align the published Notes application cohort with
  the dependency ranges verified by the coordinated Notes source update.

- [#14](https://github.com/lapismd/lapis-server/pull/14) [`73c7709`](https://github.com/lapismd/lapis-server/commit/73c7709eb36cfed6f286635a3fe726fb916b0a10) Thanks [@stevejuma](https://github.com/stevejuma)! - Publish multi-architecture server images and align the hosted login inputs with
  Design Core's neutral form chrome.

- [#32](https://github.com/lapismd/lapis-server/pull/32) [`92e1be4`](https://github.com/lapismd/lapis-server/commit/92e1be4eabf0962f95eb8e1ca5d55ffaa4a19380) Thanks [@stevejuma](https://github.com/stevejuma)! - Allow GitHub Pages deployment timeout handling and one bounded retry, and finish
  active deployments before deploying newer Storybook builds.

- [#17](https://github.com/lapismd/lapis-server/pull/17) [`8fcadb9`](https://github.com/lapismd/lapis-server/commit/8fcadb984effd5aafb0c03cf8ea0445648bdc37c) Thanks [@stevejuma](https://github.com/stevejuma)! - Include persistent Homebrew-backed Codex, Cursor, Claude Code and OpenCode
  tooling in the server image, with bottle-only operator package installation.

- [#13](https://github.com/lapismd/lapis-server/pull/13) [`3de2f98`](https://github.com/lapismd/lapis-server/commit/3de2f981b17f4832fd48ac266ba8c6c6f4ca5e4c) Thanks [@lapismd-dependency-bot](https://github.com/apps/lapismd-dependency-bot)! - Serialize ARM64 Homebrew bottle downloads and prime the tooling layer before
  multi-architecture image publication.

- [#18](https://github.com/lapismd/lapis-server/pull/18) [`774dcc7`](https://github.com/lapismd/lapis-server/commit/774dcc72ee8f641e4c65d08ac55719ebe41f6767) Thanks [@stevejuma](https://github.com/stevejuma)! - Cross-compile the ARM64 server and build static Web assets on the native build
  platform so multi-architecture image publication completes within its deadline.

## 1.0.1

### Patch Changes

- [#9](https://github.com/lapismd/lapis-server/pull/9) [`4150cac`](https://github.com/lapismd/lapis-server/commit/4150cac6f2fef28b128421e4e3a851b848acdcf3) Thanks [@stevejuma](https://github.com/stevejuma)! - Restore the retained native Markdown, Search, Tasks and Docs projection engines
  through pinned owning Git dependencies in default App Core builds. Explicit
  vault ownership and write admission remain unchanged.

- [#11](https://github.com/lapismd/lapis-server/pull/11) [`912fe10`](https://github.com/lapismd/lapis-server/commit/912fe1000d8af14bceeb99ab06ef4a2ed5ca1a22) Thanks [@stevejuma](https://github.com/stevejuma)! - Restore the branded Deno login and vault chooser, full-height shared Notes
  workspace session, production Storybook coverage and current Lapis dependencies.

## 1.0.0

### Major Changes

- Publish the hosted server image `ghcr.io/lapismd/lapis-server`. A push to main tags the image with the 7-character commit hash, and a `v*` tag publishes that release.
