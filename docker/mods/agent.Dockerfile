FROM ubuntu:24.04 AS prepare
ARG TARGETARCH
ARG AGENT
RUN apt-get update && apt-get install -y --no-install-recommends python3 ca-certificates libstdc++6 libgcc-s1 libatomic1 \
    && rm -rf /var/lib/apt/lists/*
COPY docker/mods/agents.json docker/mods/build-agent.py /inputs/
RUN python3 /inputs/build-agent.py "$AGENT" "$TARGETARCH"
FROM scratch
LABEL org.opencontainers.image.source="https://github.com/lapismd/lapis-server"
COPY --from=prepare /rootfs/ /
