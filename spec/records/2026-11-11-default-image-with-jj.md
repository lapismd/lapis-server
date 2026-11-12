# Minimal default image with JJ

SRV-SRV-001, SRV-HST-016 and SRV-GOV-001 now require a Homebrew-free default
runtime with checksum-pinned upstream jj 0.45.1. Agent distributions remain
separately selectable OCI Mods. Existing published tags remain unchanged.

The normal and -slim tags are emitted by one multiarchitecture build. The
runtime-slim build target remains compatible; default runtime inherits it.
Homebrew builders, installer, startup integration and volume declarations are
removed. Old volumes are preserved. Nonempty obsolete package requests fail
with migration guidance. No npm or crate publication is authorized.

JJ uses the existing CI installer pins and archive validation, downloaded on
the build platform for the target architecture; only the executable enters
the final image. Runtime checks require its version and actual repository
initialization/status, absent brew/agents/compiler, original entrypoint,
Mod executable/init/post hooks, UID 1993, read-only no-Mod boot, graceful stop
and the unchanged 500 MiB unpacked limit.

## Qualification

The original public 43d6657 default image fails the unchanged 500 MiB smoke
limit at 2,355,658,752 unpacked bytes. The default checkout target builds on
remote Docker context omarchy and passes the complete runtime smoke lane at
294,002,688 unpacked bytes: jj version 0.45.1 and actual repository init/status,
absent Homebrew/agent distributions/compiler, read-only no-Mod readiness,
real executable and init/post hooks, PID1 UID 1993 and graceful shutdown.
Receipts: `.local/default-image-jj/build-amd64.log`, `before-default.log`
and `runtime-amd64.log`. This is AMD64 source-image evidence; hosted ARM64
qualification and actual public artifact acceptance remain pending.

Actionlint passes for every workflow, all eight demo regressions pass, root
and native Compose examples interpolate on omarchy, Prettier passes, and the
latest spec-validator 0.2.0 doctor/search/check and App ownership audit pass.
Doctor retains its existing custom-script-alias warning.

Separate live-request diagnosis on the unchanged public 43d6657 image confirms
112 commands/77 database requests in the 600-dormant-assets/core fixture.
The live user vault has recursive polling and broader enabled-plugin startup
traffic; that issue is not qualified by the dormant fixture or this image slice.

## JJ Git transport dependency

The first minimal candidate passed jj init/status but a real local clone fails
with `Could not execute the git process`. Git is therefore installed through
APT alongside the pinned JJ executable. The runtime smoke now requires local
clone/status as well; final size and smoke requalification follow this change.

Final AMD64 runtime (with Git) passes the complete smoke lane at 376,160,256
unpacked bytes, including a real local jj git clone/status. The clone fixture
uses the initialized working repository rather than assuming JJ's private store
layout. Receipt: `.local/default-image-jj/runtime-with-git.log` in the owner
checkout. The earlier 294 MB measurement is a superseded no-Git candidate.
