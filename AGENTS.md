# lapis-server

This repository owns the hosted server image `ghcr.io/lapismd/lapis-server`.
Notes owns the public API packages and does not run server acceptance.
Read the current contract, verification and `spec/src/backlog.md`; keep remaining
work in the owning spec topic instead of a migration or milestone tracker.

jj-store crates use `git = "https://github.com/stevejuma/jj-store"` and
`rev = "076e418cecbe204925aea874bef8aa8a680269a9"`. Do not invent a jj-store
tag. `lapis-ai-service` stays tag `v0.2.0`.

Use Jujutsu. Do not publish crates. A push to `main` publishes
`ghcr.io/lapismd/lapis-server` tagged with the 7-character
`git rev-parse --short` hash. Changesets record the release note, open the
version pull request, and create the `v*` tag and GitHub release. That tag
also publishes the image. Do not publish an npm package.

Use the installed `spec-validator` skill. Prefer
`pnpm spec:search "<topic or ID>"` before any manual scan of `spec/src`.
Use `--semantic` for conceptual queries. Use `rg` only when QMD is unavailable.
