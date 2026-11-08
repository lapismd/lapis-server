#!/bin/sh
set -eu

/app/install-homebrew-packages
exec /app/lapis-server "$@"
