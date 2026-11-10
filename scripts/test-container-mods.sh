#!/bin/sh
set -eu
# Build contexts are sent to the selected engine; no host bind mounts or socket mounts.
context=${LAPIS_DOCKER_CONTEXT:-default}
docker_mods() { docker --context "$context" "$@"; }
loader=${1:-lapis-mod-loader:qualification}
prefix="lapis-mods-test-$$"
network="$prefix-network"
cache="$prefix-cache"
cleanup() {
  docker_mods rm --force "$prefix-app" "$prefix-registry" >/dev/null 2>&1 || true
  docker_mods volume rm "$cache" >/dev/null 2>&1 || true
  docker_mods network rm "$network" >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM
docker_mods network create "$network" >/dev/null
docker_mods volume create "$cache" >/dev/null
docker_mods run --detach --name "$prefix-registry" --network "$network" \
  --network-alias mod-registry --publish 127.0.0.1::5000 registry:3 >/dev/null
port=$(docker_mods port "$prefix-registry" 5000/tcp | sed 's/.*://')
# Registry pushes go through the selected engine; tags are test-local, never public.
for sample in tool hook; do
  docker_mods build --load --tag "localhost:$port/mods:$sample" "docker/mods/examples/$sample" >/dev/null
  docker_mods push "localhost:$port/mods:$sample" >/dev/null
done
for base in alpine:3.23 ubuntu:24.04; do
  tag=$(printf '%s' "$base" | tr ':/' '--')
  docker_mods build --load --build-arg "LOADER_IMAGE=$loader" --build-arg "BASE_IMAGE=$base" \
    --file docker/mod-loader/test-image.Dockerfile --tag "lapis-mods-application:$tag" . >/dev/null
  # No-Mod operation preserves argv and works on a read-only root as an ordinary user.
  docker_mods run --rm --read-only --user 1993:1993 --entrypoint /usr/local/bin/mod-loader \
    "lapis-mods-application:$tag" -- /bin/sh -c 'test "$1" = "literal; argument"' sh 'literal; argument'
  # Fresh containers share verified downloads, but each receives its own live filesystem.
  for attempt in 1 2; do
    if ! output=$(docker_mods run --rm --network "$network" --volume "$cache:/var/cache/container-mods" \
      --env 'DOCKER_MODS=mod-registry:5000/mods:tool|mod-registry:5000/mods:hook' \
      --env DOCKER_MODS_INSECURE_REGISTRIES=mod-registry:5000 \
      "lapis-mods-application:$tag" 2>&1); then printf '%s\n' "$output"; exit 1; fi
    printf '%s\n' "$output"
    printf '%s' "$output" | grep -q original-entrypoint-ran
    if [ "$attempt" = 2 ]; then printf '%s' "$output" | grep -q cache-hit; fi
  done
  output=$(docker_mods run --rm --network none --volume "$cache:/var/cache/container-mods" \
    --env 'DOCKER_MODS=mod-registry:5000/mods:tool|mod-registry:5000/mods:hook' \
    --env DOCKER_MODS_OFFLINE=true "lapis-mods-application:$tag" 2>&1)
  printf '%s' "$output" | grep -q original-entrypoint-ran
  for restriction in --read-only --user=1993:1993; do
    if output=$(docker_mods run --rm "$restriction" \
      --env DOCKER_MODS=mod-registry:5000/mods:tool "lapis-mods-application:$tag" 2>&1); then
      echo 'Unsupported Mod privilege/filesystem configuration was accepted' >&2; exit 1
    fi
    printf '%s' "$output" | grep -Eq 'require root|writable root filesystem'
  done
  docker_mods run --detach --name "$prefix-app" --network "$network" \
    --volume "$cache:/var/cache/container-mods" \
    --env 'DOCKER_MODS=mod-registry:5000/mods:tool|mod-registry:5000/mods:hook' \
    --env DOCKER_MODS_INSECURE_REGISTRIES=mod-registry:5000 --env MOD_TEST_WAIT=1 \
    "lapis-mods-application:$tag" >/dev/null
  ready=false
  for attempt in $(seq 1 30); do
    if docker_mods logs "$prefix-app" 2>&1 | grep -q original-entrypoint-ran; then ready=true; break; fi
    sleep 1
  done
  test "$ready" = true
  docker_mods restart --time 5 "$prefix-app" >/dev/null
  # restart returns before initialization/exec completes. Wait for this boot's
  # readiness marker rather than stopping the loader (whose correct exit is 143).
  ready=false
  for attempt in $(seq 1 30); do
    boots=$(docker_mods logs "$prefix-app" 2>&1 | grep -c '^original-entrypoint-ran$' || true)
    if [ "$boots" -ge 2 ]; then ready=true; break; fi
    sleep 1
  done
  test "$ready" = true
  docker_mods stop --time 5 "$prefix-app" >/dev/null
  docker_mods logs "$prefix-app" | grep -q original-entrypoint-sigterm
  test "$(docker_mods inspect --format '{{.State.ExitCode}}' "$prefix-app")" = 0
  docker_mods rm "$prefix-app" >/dev/null
  docker_mods run --detach --name "$prefix-app" --network "$network" \
    --volume "$cache:/var/cache/container-mods" \
    --env DOCKER_MODS=mod-registry:5000/mods:hook \
    --env DOCKER_MODS_INSECURE_REGISTRIES=mod-registry:5000 --env MOD_TEST_INIT_WAIT=1 \
    "lapis-mods-application:$tag" >/dev/null
  waiting=false
  for attempt in $(seq 1 30); do
    if docker_mods logs "$prefix-app" 2>&1 | grep -q initialization-hook-waiting; then waiting=true; break; fi
    sleep 1
  done
  test "$waiting" = true
  docker_mods stop --time 5 "$prefix-app" >/dev/null
  test "$(docker_mods inspect --format '{{.State.ExitCode}}' "$prefix-app")" = 143
  if docker_mods logs "$prefix-app" 2>&1 | grep -q original-entrypoint-ran; then exit 1; fi
  docker_mods rm "$prefix-app" >/dev/null
done
# Optional real agent Mod qualification; each distribution is injected without Homebrew.
for agent in ${LAPIS_MOD_TEST_AGENTS:-}; do
  case "$agent" in
    codex) executable=codex ;;
    cursor) executable=cursor-agent ;;
    claude) executable=claude ;;
    opencode) executable=opencode ;;
    *) echo 'Unknown agent qualification target' >&2; exit 64 ;;
  esac
  docker_mods build --load --build-arg "AGENT=$agent" --file docker/mods/agent.Dockerfile \
    --tag "localhost:$port/mods:$agent" . >/dev/null
  docker_mods push "localhost:$port/mods:$agent" >/dev/null
  docker_mods run --rm --network "$network" --volume "$cache:/var/cache/container-mods" \
    --env HOME=/tmp/agent-mod-home \
    --env "DOCKER_MODS=mod-registry:5000/mods:$agent" \
    --env DOCKER_MODS_INSECURE_REGISTRIES=mod-registry:5000 \
    --env OPENCODE_DISABLE_AUTOUPDATE=1 --env OPENCODE_DISABLE_MODELS_FETCH=1 \
    --entrypoint /usr/local/bin/mod-loader lapis-mods-application:ubuntu-24.04 \
    --uid 1993 --gid 1993 -- /usr/bin/timeout 60 "/usr/local/bin/$executable" --version
  # Retain the qualified image for artifact publication, not a second rebuild.
  docker_mods tag "localhost:$port/mods:$agent" "lapis-agent-mod:$agent"
done
