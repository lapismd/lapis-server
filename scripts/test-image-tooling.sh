#!/bin/sh
set -eu

image=${1:?Usage: scripts/test-image-tooling.sh IMAGE [--persistence]}
mode=${2:-}

docker run --rm --entrypoint /bin/sh "$image" -c '
  set -eu
  test "$(id -u)" = 1993
  test "$(id -g)" = 1993
  test -w /home/linuxbrew/.linuxbrew
  ! command -v cc >/dev/null 2>&1
  ! command -v gcc >/dev/null 2>&1
  ! command -v node >/dev/null 2>&1
  ! command -v deno >/dev/null 2>&1
  brew --version
  codex --version
  cursor-agent --version
  claude --version
  opencode --version
'

cask_packages=$(printf 'codex\nclaude-code')
docker run --rm \
  --env "LAPIS_SERVER_HOMEBREW_CASKS=$cask_packages" \
  --entrypoint /app/install-homebrew-packages \
  "$image"

if docker run --rm \
  --env 'LAPIS_SERVER_HOMEBREW_PACKAGES=jq;false' \
  --entrypoint /app/install-homebrew-packages \
  "$image"; then
  echo "Unsafe Homebrew package input was accepted" >&2
  exit 1
fi

if docker run --rm \
  --env 'LAPIS_SERVER_HOMEBREW_PACKAGES=../jq' \
  --entrypoint /app/install-homebrew-packages \
  "$image"; then
  echo "Path-like Homebrew package input was accepted" >&2
  exit 1
fi

masked_output=$(docker run --rm \
  --tmpfs /home/linuxbrew/.linuxbrew \
  --entrypoint /app/install-homebrew-packages \
  "$image" 2>&1) && {
  echo "A masked Homebrew prefix was accepted" >&2
  exit 1
}
case "$masked_output" in
  *"Homebrew is unavailable"*) ;;
  *)
    echo "A masked Homebrew prefix did not report a clear error" >&2
    echo "$masked_output" >&2
    exit 1
    ;;
esac

[ "$mode" = --persistence ] || exit 0

volume="lapis-server-homebrew-test-$$"
cleanup() {
  docker volume rm "$volume" >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM
docker volume create "$volume" >/dev/null

docker run --rm \
  --volume "$volume":/home/linuxbrew/.linuxbrew \
  --env LAPIS_SERVER_HOMEBREW_PACKAGES=jq \
  --entrypoint /app/install-homebrew-packages \
  "$image"
docker run --rm \
  --volume "$volume":/home/linuxbrew/.linuxbrew \
  --entrypoint /bin/sh \
  "$image" -c 'jq --version && brew list --formula jq >/dev/null'
