# @lapismd/lapis-server

## 1.0.2

### Patch Changes

- [#16](https://github.com/lapismd/lapis-server/pull/16) [`fd92fb5`](https://github.com/lapismd/lapis-server/commit/fd92fb521f9ac49a20bb97293c32f50c5e31cbaa) Thanks [@stevejuma](https://github.com/stevejuma)! - Extend the opt-in debug backup diagnostics across shutdown and final capture
  with fixed safe categories while preserving public errors and backup behavior.

- [#14](https://github.com/lapismd/lapis-server/pull/14) [`73c7709`](https://github.com/lapismd/lapis-server/commit/73c7709eb36cfed6f286635a3fe726fb916b0a10) Thanks [@stevejuma](https://github.com/stevejuma)! - Publish multi-architecture server images and align the hosted login inputs with
  Design Core's neutral form chrome.

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
