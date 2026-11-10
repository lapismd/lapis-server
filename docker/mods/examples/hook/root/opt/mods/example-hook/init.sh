#!/bin/sh
set -eu
mkdir -p /opt/mods/example-hook
printf 'initialized\n' > /opt/mods/example-hook/marker
if [ "${MOD_TEST_INIT_WAIT:-0}" = 1 ]; then
  printf 'initialization-hook-waiting\n'
  while :; do sleep 1; done
fi
