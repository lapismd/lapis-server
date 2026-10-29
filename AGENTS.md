# lapis-server

This repository owns the hosted server image `ghcr.io/lapismd/lapis-server`.
Notes owns the public API packages and does not run server acceptance.

jj-store crates use `git = "https://github.com/stevejuma/jj-store"` and
`rev = "7a98bd89d2bc77fb6ec41f91186992abb19ba161"`. Do not invent a jj-store
tag. `lapis-ai-service` stays tag `v0.2.0`.

Use Jujutsu. Do not publish crates or push the image without a separate
release authorization. A git tag `v*` is the image release. Do not use
Changesets and do not publish an npm package.

Use the installed `spec-validator` skill. Prefer
`pnpm spec:search "<topic or ID>"` before any manual scan of `spec/src`.
Use `--semantic` for conceptual queries. Use `rg` only when QMD is unavailable.
