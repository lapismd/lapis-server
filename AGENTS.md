# lapis-server

This repository owns the hosted server image `ghcr.io/lapismd/lapis-server`.
Notes owns the public API packages and does not run server acceptance.

jj-store crates stay path dependencies until
https://github.com/stevejuma/jj-store/pull/22 merges. Do not point them at
that pull request's head. Do not invent a jj-store tag. `lapis-ai-service`
stays tag `v0.2.0`.

Use Jujutsu. Do not publish crates or push the image without a separate
release authorization.
