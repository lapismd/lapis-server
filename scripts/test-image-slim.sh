#!/bin/sh
set -eu

image=${1:?Usage: scripts/test-image-slim.sh IMAGE}
# Honor the caller's context explicitly; CI defaults to its local engine.
context=${LAPIS_DOCKER_CONTEXT:-default}
docker_local() { docker --context "$context" "$@"; }
size=$(docker_local image inspect --format '{{.Size}}' "$image")
[ "$size" -lt 524288000 ] || { echo "Slim image exceeds 500 MiB: $size bytes" >&2; exit 1; }
docker_local run --rm --entrypoint /bin/sh "$image" -c '
  set -eu
  test "$(id -u)" = 1993
  test "$(id -g)" = 1993
  test -s /app/static/index.html
  test -x /app/lapis-server
  ! test -w /app/lapis-server
  ! test -w /app/static/index.html
  for tool in brew codex cursor-agent claude opencode node deno cc; do
    ! command -v "$tool" >/dev/null 2>&1
  done
  touch /data/slim-acceptance
'
output=$(docker_local run --rm --env LAPIS_SERVER_HOMEBREW_PACKAGES=jq "$image" 2>&1) && {
  echo 'Slim image ignored a requested Homebrew install' >&2; exit 1;
}
case "$output" in
  *'use the full image'*) ;;
  *) echo "$output" >&2; exit 1 ;;
esac
fixture=$(mktemp -d)
printf '{"vaults":[]}\n' > "$fixture/config.json"
printf 'slim-image-isolated-test-password\n' > "$fixture/password"
name="lapis-slim-acceptance-$$"
cleanup() { docker_local rm --force --volumes "$name" >/dev/null 2>&1 || true; rm -rf "$fixture"; }
trap cleanup EXIT INT TERM
# Only this disposable test container and its anonymous volume are removed.
docker_local run --detach --name "$name" --read-only --tmpfs /tmp \
  --env LAPIS_SERVER_ORIGIN=http://localhost:8787 \
  --env LAPIS_SERVER_INSECURE_COOKIE=1 \
  --env LAPIS_SERVER_BOOTSTRAP_USERNAME=owner \
  --env LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE=/run/secrets/password \
  --volume "$fixture/password:/run/secrets/password:ro" \
  --volume "$fixture/config.json:/run/lapis/config.json:ro" \
  "$image" >/dev/null
ready=false
for attempt in $(seq 1 30); do
  if [ "$(docker_local inspect --format '{{.State.Running}}' "$name")" != true ]; then
    docker_local logs "$name"; exit 1
  fi
  if docker_local exec "$name" curl --fail --silent http://127.0.0.1:8787/readyz >/dev/null; then
    ready=true; break
  fi
  sleep 1
done
if [ "$ready" != true ]; then docker_local logs "$name"; exit 1; fi
printf 'Slim image passed readiness, permissions, tooling and size checks (%s bytes).\n' "$size"
