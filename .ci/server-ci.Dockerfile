# syntax=docker/dockerfile:1.7

FROM rust:1.99.0-slim-bookworm@sha256:2c3a22f0a5533ea2dd5a16627bc841228151faa2d4de2644ac9987e4a2f1f2fa AS rust
FROM ghcr.io/lapismd/lapis-ci@sha256:91341a81cc9306eed818c4c433cd29ed11d2ea64c703f32685391befe0eca503

LABEL org.opencontainers.image.title="Lapis Server CI" \
      org.opencontainers.image.description="Pinned Server CI toolchain without repository source or dependency outputs" \
      org.opencontainers.image.source="https://github.com/lapismd/lapis-server" \
      org.opencontainers.image.licenses="Apache-2.0"

COPY --from=rust /usr/local/cargo /usr/local/cargo
COPY --from=rust /usr/local/rustup /usr/local/rustup
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/cache/cargo \
    CARGO_BUILD_JOBS=2 \
    PATH=/usr/local/cargo/bin:/pnpm:$PATH

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    build-essential \
    clang \
    cmake \
    libssl-dev \
    pkg-config \
  && rm -rf /var/lib/apt/lists/* \
  && rustup component add clippy rustfmt --toolchain 1.99.0

COPY scripts/ci-setup-jj.mjs /usr/local/share/lapis-server-ci/ci-setup-jj.mjs
COPY scripts/ci-image-tools.mjs /usr/local/share/lapis-server-ci/ci-image-tools.mjs
COPY .ci/server-ci.json /usr/local/share/lapis-server-ci/server-ci.json
RUN node /usr/local/share/lapis-server-ci/ci-image-tools.mjs install \
  /usr/local/share/lapis-server-ci/server-ci.json

WORKDIR /workspace
