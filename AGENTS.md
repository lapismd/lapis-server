# lapis-server

This repository owns the hosted server image `ghcr.io/lapismd/lapis-server`.
Notes owns the public API packages and does not run server acceptance.

jj-store crates use `git = "https://github.com/stevejuma/jj-store"` and
`rev = "7a98bd89d2bc77fb6ec41f91186992abb19ba161"`. Do not invent a jj-store
tag. `lapis-ai-service` stays tag `v0.2.0`.

Use Jujutsu. Do not publish crates or push the image without a separate
release authorization.
