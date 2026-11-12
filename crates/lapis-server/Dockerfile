# syntax=docker/dockerfile:1.7
ARG MOD_LOADER_IMAGE=ghcr.io/lapismd/mod-loader:latest
FROM ${MOD_LOADER_IMAGE} AS mod-loader
FROM --platform=$BUILDPLATFORM rust:1.99.0-bookworm AS native
ARG BUILDARCH
ARG TARGETARCH
RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake clang pkg-config libssl-dev \
    && if [ "$BUILDARCH" = amd64 ] && [ "$TARGETARCH" = arm64 ]; then \
         apt-get install -y --no-install-recommends gcc-aarch64-linux-gnu g++-aarch64-linux-gnu; \
         rustup target add aarch64-unknown-linux-gnu; \
       fi \
    && rm -rf /var/lib/apt/lists/*
COPY . /work/lapis-server
# A lapis-server checkout is the build context. Qualification may stage
# path-pinned sibling crates and the archive manifest inside .image-inputs;
# promote them beside the repository before compilation.
RUN set -eu; \
    sib=/work/lapis-server/.image-inputs/siblings; \
    qual=/work/lapis-server/.image-inputs/qualification; \
    if [ -d "$sib" ]; then find "$sib" -mindepth 1 -maxdepth 1 -exec mv {} /work/ \;; fi; \
    if [ -d "$qual" ]; then mv "$qual" /work/.qualification; fi; \
    rm -rf /work/lapis-server/.image-inputs
WORKDIR /work/lapis-server
ENV CARGO_BUILD_JOBS=2
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++
RUN --mount=type=cache,id=lapis-server-rust-cargo-${TARGETARCH},target=/usr/local/cargo/registry \
    --mount=type=cache,id=lapis-server-rust-git-${TARGETARCH},target=/usr/local/cargo/git \
    --mount=type=cache,id=lapis-server-rust-release-${TARGETARCH},target=/work/lapis-server/.local/rust-core-target \
    case "$TARGETARCH" in \
      amd64) target=x86_64-unknown-linux-gnu ;; \
      arm64) target=aarch64-unknown-linux-gnu ;; \
      *) echo "Unsupported target architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && cargo build --release --locked -p lapis-server --target "$target" \
    && cp ".local/rust-core-target/$target/release/lapis-server" /lapis-server

FROM --platform=$BUILDPLATFORM node:24.15.0-bookworm-slim AS web
ARG TARGETARCH
RUN corepack enable && corepack prepare pnpm@10.34.5 --activate
COPY . /work/lapis-server
# A lapis-server checkout is the build context. Qualification may stage
# path-pinned sibling crates and the archive manifest inside .image-inputs;
# promote them beside the repository before compilation.
RUN set -eu; \
    sib=/work/lapis-server/.image-inputs/siblings; \
    qual=/work/lapis-server/.image-inputs/qualification; \
    if [ -d "$sib" ]; then find "$sib" -mindepth 1 -maxdepth 1 -exec mv {} /work/ \;; fi; \
    if [ -d "$qual" ]; then mv "$qual" /work/.qualification; fi; \
    rm -rf /work/lapis-server/.image-inputs
WORKDIR /work/lapis-server
ENV NODE_OPTIONS=--max-old-space-size=8192 GOMAXPROCS=2 UV_THREADPOOL_SIZE=2
RUN --mount=type=cache,id=lapis-server-rust-pnpm-${TARGETARCH},target=/cache/pnpm pnpm install --frozen-lockfile --store-dir /cache/pnpm
RUN pnpm exec vite build --config web/vite.config.ts

# Download target-platform JJ using the same checked upstream pins as CI.
FROM --platform=$BUILDPLATFORM node:24.15.0-bookworm-slim AS jj-tool
ARG TARGETARCH
COPY scripts/ci-setup-jj.mjs /tmp/ci-setup-jj.mjs
RUN TARGETARCH=$TARGETARCH node --input-type=module <<'JS'
import { jjArtifact, verifyJj, jjArchiveMember } from "/tmp/ci-setup-jj.mjs";
import { mkdirSync, writeFileSync, chmodSync } from "node:fs";
import { execFileSync } from "node:child_process";
const arch = { amd64: "x64", arm64: "arm64" }[process.env.TARGETARCH];
const { name, digest } = jjArtifact("linux", arch);
const response = await fetch(`https://github.com/jj-vcs/jj/releases/download/v0.45.1/${name}`);
if (!response.ok) throw Error(`JJ download failed: HTTP ${response.status}`);
const bytes = Buffer.from(await response.arrayBuffer());
verifyJj(bytes, digest);
mkdirSync("/out");
writeFileSync("/tmp/jj.tar.gz", bytes);
const member = jjArchiveMember(execFileSync("tar", ["-tzf", "/tmp/jj.tar.gz"], { encoding: "utf8" }));
execFileSync("tar", ["-xzf", "/tmp/jj.tar.gz", "-C", "/out", member]);
chmodSync("/out/jj", 0o555);
JS

# The default and compatibility -slim tags share one Homebrew-free runtime.
FROM ubuntu:24.04 AS runtime-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl git libgcc-s1 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 1993 lapis \
    && useradd --uid 1993 --gid 1993 --home-dir /data --shell /bin/sh lapis \
    && mkdir -p /app /data && chown lapis:lapis /data
COPY --from=mod-loader /usr/local/bin/mod-loader /usr/local/bin/mod-loader
COPY --from=native --chmod=0555 /lapis-server /app/lapis-server
COPY --from=web --chmod=0555 /work/lapis-server/web/dist /app/static
COPY --from=jj-tool --chmod=0555 /out/jj /usr/local/bin/jj
COPY --chmod=0555 crates/lapis-server/container-entrypoint.sh /app/container-entrypoint
ENV HOME=/data LAPIS_SERVER_DATA_ROOT=/data LAPIS_SERVER_STATIC_ROOT=/app/static LAPIS_SERVER_NATIVE_HISTORY=1
USER lapis
WORKDIR /app
EXPOSE 8787
VOLUME ["/data"]
HEALTHCHECK --interval=30s --timeout=3s --start-period=15s --retries=3 CMD ["curl", "--fail", "--silent", "http://127.0.0.1:8787/readyz"]
ENTRYPOINT ["/usr/local/bin/mod-loader", "--uid", "1993", "--gid", "1993", "--", "/app/container-entrypoint"]
CMD ["/run/lapis/config.json", "0.0.0.0:8787"]

FROM runtime-slim AS runtime
