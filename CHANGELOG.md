# @lapismd/lapis-server

## 1.0.1

### Patch Changes

- [#9](https://github.com/lapismd/lapis-server/pull/9) [`4150cac`](https://github.com/lapismd/lapis-server/commit/4150cac6f2fef28b128421e4e3a851b848acdcf3) Thanks [@stevejuma](https://github.com/stevejuma)! - Restore the retained native Markdown, Search, Tasks and Docs projection engines
  through pinned owning Git dependencies in default App Core builds. Explicit
  vault ownership and write admission remain unchanged.

## 1.0.0

### Major Changes

- Publish the hosted server image `ghcr.io/lapismd/lapis-server`. A push to main tags the image with the 7-character commit hash, and a `v*` tag publishes that release.
