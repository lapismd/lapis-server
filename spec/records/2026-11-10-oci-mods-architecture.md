# OCI runtime Mods architecture

## Decision and prior art

Studied LinuxServer's `mod-scripts` branch, `docker-mods.v3` version
3.20250825 and its Alpine base's `S6_STAGE2_HOOK=/docker-mods`. Its loader runs
before s6 initialization, downloads the first filesystem layer, caches by digest
and copies an extracted payload into the live root. s6 then orders setup and
custom services. The `init-mods` oneshot marks that phase's dependencies; s6 is
not necessary for a single foreground application's entrypoint.

Sources reviewed:

- <https://github.com/linuxserver/docker-mods/blob/mod-scripts/docker-mods.v3>
- <https://github.com/linuxserver/docker-baseimage-alpine/blob/master/Dockerfile>
- <https://github.com/linuxserver/docker-baseimage-alpine/tree/master/root/etc/s6-overlay/s6-rc.d/init-mods>
- <https://github.com/oras-project/rust-oci-client>

Use an independent, static Rust-musl `mod-loader` binary in a scratch OCI image,
exported at `/usr/local/bin/mod-loader` for multi-stage COPY into any base.
The crate has its own workspace and Cargo.lock, with no Lapis runtime dependency.
ORAS `oci-client` 0.18.0 owns the registry protocol; `docker_credential` supports
standard configured registry secrets/helpers. `cap-std` constrains filesystem
operations. Reuse the existing foreground entrypoint through exec, with explicit
privilege drop flags; introduce neither s6 nor a daemon or Docker socket.

Native AMD64 and ARM64 CI qualifies the artifact once, and publication loads
those exact artifacts instead of rebuilding. The image release supplies the
resulting immutable loader digest to both full and minimal server builds. PR
image builds use the same qualified artifact as a named OCI build context.
Each agent is a separate scratch Mod built from complete, checksum-pinned
upstream distributions and ordinary runtime libraries, without Homebrew.

## Boundaries and trade-offs

Mods are trusted initialization code, not sandboxed application plugins. The
loader never escalates privileges. Existing images retain their normal non-root
USER and original entrypoint behavior when DOCKER_MODS is empty. Operators must
explicitly select root initialization and a writable container layer for Mods;
Lapis drops supplemental groups and UID/GID to 1993 before its original startup.
The base image supplies CA certificates; a static binary does not imply that
all agent payloads share its libc independence. Agent qualification is separate
from Alpine/Ubuntu loader qualification.

Files are added to the live container filesystem, not committed as Docker
image layers. Downloads are independently cached with a root-owned shared lock;
mutable tags resolve online every start. Explicit offline operation requires
verified cached manifests/config/layers. Receipts are container-local so a new
container re-applies files even when it reuses the download cache.

Initial format: one raw/gzip filesystem layer. Explicitly reject other layer
counts, GNU/PAX extensions, whiteouts, devices, hardlinks, traversal and unsafe
links or permission bits. Existing files need exact replacement declarations.
Preflight and per-file atomic replacement/rollback limit partial filesystem
changes; newly created empty directories may remain. Hooks run only after file
application and cannot be transactionally undone. No background service support
or implicit package-manager invocation is added.

The historical full Homebrew image remains available. The small image carries
the loader; deployments select only the desired agent Mods. This avoids making
every application's base image ship every agent runtime, while preserving the
existing full-image compatibility contract.
