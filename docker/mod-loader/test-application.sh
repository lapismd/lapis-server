#!/bin/sh
set -eu
mod-example-tool
test "$(cat /opt/mods/example-hook/marker)" = "$(printf 'initialized\npost-initialized')"
if [ "${MOD_TEST_WAIT:-0}" = 1 ]; then
  trap 'printf "original-entrypoint-sigterm\n"; exit 0' TERM
fi
printf 'original-entrypoint-ran\n'
if [ "${MOD_TEST_WAIT:-0}" = 1 ]; then
  while :; do sleep 1 & wait $! || true; done
fi
