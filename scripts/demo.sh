#!/bin/sh
set -eu

# Resolve the checkout even when invoked from another working directory.
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
command -v docker >/dev/null 2>&1 || { echo "Docker with Compose is required." >&2; exit 1; }
command -v openssl >/dev/null 2>&1 || { echo "OpenSSL is required to generate the demo password." >&2; exit 1; }

# Never follow the selected context to a remote daemon.
context=${LAPIS_DEMO_DOCKER_CONTEXT:-default}

action=${1:-up}
case "$action" in
  up|prepare|image) ;;
  *) echo "Usage: $0 [up|prepare|image IMAGE]" >&2; exit 2 ;;
esac

if [ "$action" = image ]; then
  if [ -z "${2:-}" ]; then
    echo "Usage: $0 image ghcr.io/lapismd/lapis-server:TAG" >&2
    exit 2
  fi
  export LAPIS_SERVER_IMAGE=$2
  docker --context "$context" info >/dev/null
fi

if [ "$action" = up ]; then
  memory=$(docker --context "$context" info --format '{{.MemTotal}}')
  if [ "$memory" -lt 5368709120 ]; then
    echo "Allocate at least 6 GiB RAM to Docker before building the demo from source." >&2
    exit 1
  fi
fi

umask 077
if [ ! -e .env ]; then
  cp .env.example .env
  echo "Created .env from .env.example."
fi
mkdir -p .local/demo
if [ ! -e .local/demo/config.json ]; then
  printf '%s\n' '{"vaults":[]}' > .local/demo/config.json
  chmod 644 .local/demo/config.json
fi
if [ ! -e .local/demo/bootstrap-password ]; then
  openssl rand -hex 24 > .local/demo/bootstrap-password
  # The non-root container must read the mounted file. Its host directory is private.
  chmod 700 .local/demo
  chmod 644 .local/demo/bootstrap-password
fi

docker --context "$context" compose config --quiet
if [ "$action" = prepare ]; then
  echo "Demo configured. Password file: .local/demo/bootstrap-password"
  exit 0
fi

if [ "$action" = image ]; then
  docker --context "$context" compose pull
  docker --context "$context" compose up --no-build --detach --wait --wait-timeout 180
else
  # Prime the expensive stages serially so a small local Docker VM does not run
  # the Rust compiler and Vite bundler together. Compose reuses these build layers.
  for stage in web native; do
    docker --context "$context" build --file crates/lapis-server/Dockerfile --target "$stage" .
  done

  docker --context "$context" compose up --build --detach --wait --wait-timeout 180
fi
echo "Demo ready. Open LAPIS_SERVER_ORIGIN from .env (default: http://localhost:8787)."
echo "Sign in as LAPIS_SERVER_BOOTSTRAP_USERNAME (default: owner)."
echo "Read the initial password from LAPIS_SERVER_PASSWORD_FILE (default: .local/demo/bootstrap-password)."
