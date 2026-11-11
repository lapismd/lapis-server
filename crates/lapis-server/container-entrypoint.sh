#!/bin/sh
set -eu
if [ -n "${LAPIS_SERVER_HOMEBREW_PACKAGES:-}${LAPIS_SERVER_HOMEBREW_CASKS:-}" ]; then
  echo "Homebrew has been removed from the server image; configure tools with DOCKER_MODS instead." >&2
  exit 64
fi
exec /app/lapis-server "$@"
