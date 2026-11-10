#!/bin/sh
set -eu
test "$(cat /opt/mods/example-hook/marker)" = initialized
printf 'post-initialized\n' >> /opt/mods/example-hook/marker
