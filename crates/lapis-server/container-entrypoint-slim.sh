#!/bin/sh
set -eu
if [ -n "${LAPIS_SERVER_HOMEBREW_PACKAGES:-}${LAPIS_SERVER_HOMEBREW_CASKS:-}" ]; then
  echo "Homebrew packages are unavailable in the slim image; use the full image." >&2
  exit 64
fi
exec /app/lapis-server "$@"
