ARG LOADER_IMAGE=lapis-mod-loader:qualification
ARG BASE_IMAGE=alpine:3.23
FROM ${LOADER_IMAGE} AS mod-loader
FROM ${BASE_IMAGE}
RUN if command -v apk >/dev/null; then apk add --no-cache ca-certificates; else apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*; fi
COPY --from=mod-loader /usr/local/bin/mod-loader /usr/local/bin/mod-loader
COPY --chmod=0555 docker/mod-loader/test-application.sh /original-entrypoint
ENTRYPOINT ["/usr/local/bin/mod-loader", "--", "/original-entrypoint"]
