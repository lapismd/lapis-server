# Independently versioned static loader artifact; no application/base-image ABI.
FROM rust:1.99.0-alpine3.23 AS build
RUN apk add --no-cache build-base cmake perl binutils
WORKDIR /build
COPY crates/lapis-container-mods ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release --locked \
    && ! readelf -l target/release/mod-loader | grep 'INTERP' \
    && cp target/release/mod-loader /mod-loader
FROM scratch
LABEL org.opencontainers.image.source="https://github.com/lapismd/lapis-server"
COPY --from=build --chmod=0555 /mod-loader /usr/local/bin/mod-loader
