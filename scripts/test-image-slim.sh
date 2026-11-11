#!/bin/sh
set -eu
image=${1:?Usage: scripts/test-image-slim.sh IMAGE}
context=${LAPIS_DOCKER_CONTEXT:-default}
docker_slim() { docker --context "$context" "$@"; }
# History reports unpacked layer bytes consistently across classic/containerd stores.
size=$(docker_slim history --human=false --format '{{.Size}}' "$image" | awk '{ total += $1 } END { printf "%.0f", total }')
architecture=$(docker_slim image inspect --format '{{.Architecture}}' "$image")
[ "$size" -lt 524288000 ] || { echo "Slim image exceeds 500 MiB: $size bytes" >&2; exit 1; }
docker_slim run --rm --entrypoint /bin/sh "$image" -c '
  set -eu
  test "$(id -u)" = 1993
  test -s /app/static/index.html
  test -x /app/lapis-server
  test ! -e /home/linuxbrew/.linuxbrew
  test ! -e /app/install-homebrew-packages
  jj --version | grep -E "^jj 0\.45\.1(-|$)"
  directory=$(mktemp -d)
  jj git init "$directory/repo"
  jj --repository "$directory/repo" status
  jj git clone "$directory/repo" "$directory/clone"
  jj --repository "$directory/clone" status
  rm -rf "$directory"
  ! test -w /app/lapis-server
  ! test -w /app/static/index.html
  for tool in brew codex cursor-agent claude opencode node deno cc; do
    ! command -v "$tool" >/dev/null 2>&1
  done
'
output=$(docker_slim run --rm --env LAPIS_SERVER_HOMEBREW_PACKAGES=jq "$image" 2>&1) && {
  echo 'Slim image ignored a requested Homebrew install' >&2; exit 1;
}
case "$output" in *'configure tools with DOCKER_MODS instead.'*) ;; *) echo "$output" >&2; exit 1 ;; esac
prefix="lapis-slim-test-$$"
cleanup() {
  docker_slim rm --force "$prefix-app" "$prefix-registry" >/dev/null 2>&1 || true
  docker_slim volume rm "$prefix-data" "$prefix-cache" >/dev/null 2>&1 || true
  docker_slim network rm "$prefix-network" >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM
docker_slim volume create "$prefix-data" >/dev/null
docker_slim run --rm --volume "$prefix-data:/data" --entrypoint /bin/sh "$image" -c '
  set -eu; umask 077
  printf "{\"vaults\":[]}\n" > /data/acceptance-config.json
  printf "isolated-slim-acceptance-password\n" > /data/acceptance-password
  touch /data/writable-check
'
ready() {
  for attempt in $(seq 1 30); do
    if [ "$(docker_slim inspect --format '{{.State.Running}}' "$prefix-app")" != true ]; then
      docker_slim logs "$prefix-app"; return 1
    fi
    if docker_slim exec "$prefix-app" curl --fail --silent http://127.0.0.1:8787/readyz >/dev/null; then return 0; fi
    sleep 1
  done
  docker_slim logs "$prefix-app"; return 1
}
start() {
  docker_slim run --detach --name "$prefix-app" --volume "$prefix-data:/data" \
    --env LAPIS_SERVER_ORIGIN=http://localhost:8787 --env LAPIS_SERVER_INSECURE_COOKIE=1 \
    --env LAPIS_SERVER_BOOTSTRAP_USERNAME=owner \
    --env LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE=/data/acceptance-password \
    "$@" "$image" /data/acceptance-config.json 0.0.0.0:8787 >/dev/null
  ready
  docker_slim exec "$prefix-app" sh -c 'test "$(awk '\''/^Uid:/ {print $2}'\'' /proc/1/status)" = 1993'
}
# Original read-only startup and foreground service remain intact with no Mods.
start --read-only --tmpfs /tmp
docker_slim stop --time 15 "$prefix-app" >/dev/null
test "$(docker_slim inspect --format '{{.State.ExitCode}}' "$prefix-app")" = 0
docker_slim rm "$prefix-app" >/dev/null
# Apply a real OCI executable and hook before that same original service entrypoint.
docker_slim network create "$prefix-network" >/dev/null
docker_slim volume create "$prefix-cache" >/dev/null
docker_slim run --detach --name "$prefix-registry" --network "$prefix-network" \
  --network-alias mod-registry --publish 127.0.0.1::5000 registry:3 >/dev/null
port=$(docker_slim port "$prefix-registry" 5000/tcp | sed 's/.*://')
for sample in tool hook; do
  docker_slim build --load --platform "linux/$architecture" --tag "localhost:$port/mods:$sample" "docker/mods/examples/$sample" >/dev/null
  docker_slim push "localhost:$port/mods:$sample" >/dev/null
done
start --user 0:0 --network "$prefix-network" --volume "$prefix-cache:/var/cache/container-mods" \
  --env 'DOCKER_MODS=mod-registry:5000/mods:tool|mod-registry:5000/mods:hook' \
  --env DOCKER_MODS_INSECURE_REGISTRIES=mod-registry:5000
docker_slim exec --user 1993:1993 "$prefix-app" mod-example-tool | grep -q 'OCI Mod executable is available'
docker_slim exec "$prefix-app" grep -q post-initialized /opt/mods/example-hook/marker
docker_slim stop --time 15 "$prefix-app" >/dev/null
test "$(docker_slim inspect --format '{{.State.ExitCode}}' "$prefix-app")" = 0
printf 'Slim service passed original startup, OCI Mod injection, hooks, UID, shutdown and size checks (%s bytes).\n' "$size"
