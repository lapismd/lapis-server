# lapis-server

This repository owns the hosted server image `ghcr.io/lapismd/lapis-server`.
Notes owns the public API packages and does not run server acceptance.

jj-store crates use `git = "https://github.com/stevejuma/jj-store"` and
`rev = "7a98bd89d2bc77fb6ec41f91186992abb19ba161"`. Do not invent a jj-store
tag. `lapis-ai-service` stays tag `v0.2.0`.

Use Jujutsu. Do not publish crates. A push to `main` publishes
`ghcr.io/lapismd/lapis-server` tagged with the 7-character
`git rev-parse --short` hash. Changesets record the release note, open the
version pull request, and create the `v*` tag and GitHub release. That tag
also publishes the image. Do not publish an npm package.

Use the installed `spec-validator` skill. Prefer
`pnpm spec:search "<topic or ID>"` before any manual scan of `spec/src`.
Use `--semantic` for conceptual queries. Use `rg` only when QMD is unavailable.
