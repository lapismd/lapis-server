# Final CLI runtime-image acceptance

The runtime image was rebuilt after the shell-lease correction at source commit
`3b121d21bb32ca81da84cf750661509f097a7b43`. The temporary public UI archive
projection is unchanged from the earlier bundled-CLI receipt; tracked manifests
and locks retain registry ranges.

`bash scripts/test-image-slim.sh lapis-server:cli-validation` passes the existing
startup, Mod injection/hooks, UID, shutdown and size gates plus CLI help/version
and read-only installation. Image size remains 404,303,872 bytes.

A second real-container check runs with no network, read-only root filesystem
and a private UID-1993 tmpfs home. It installs the image’s CLI with Bash PATH
setup, removes the command, holds its setup lock with `flock`, verifies PATH-only
cleanup is refused, then releases the lock and verifies cleanup succeeds. This
proves the actual final image contains the lease correction.

The full Rust source qualification remains 810 passed and 7 existing ignored;
all 25 CLI cases pass. This does not qualify ARM64 images, macOS runtime,
registry UI adoption or release publication. No deployment was performed.
