"""Prepare a hash-pinned upstream agent distribution; no Homebrew runtime."""
import glob
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tarfile
import urllib.request

agent, architecture = sys.argv[1:]
metadata = json.loads(Path('/inputs/agents.json').read_text())[agent]
source = metadata[architecture]
archive = Path('/tmp/agent-download')
request = urllib.request.Request(source['url'], headers={'User-Agent': 'lapis-agent-mod-builder/0.1'})
with urllib.request.urlopen(request, timeout=120) as response, archive.open('wb') as output:
    shutil.copyfileobj(response, output)
with archive.open('rb') as content:
    actual = hashlib.file_digest(content, 'sha256').hexdigest()
if actual != source['sha256']:
    raise SystemExit('Agent archive digest mismatch')
root = Path('/rootfs')
destination = root / 'opt/agents' / agent
destination.mkdir(parents=True)
if metadata['format'] == 'tar.gz':
    with tarfile.open(archive, 'r:gz') as content:
        content.extractall(destination, filter='data')
else:
    shutil.copyfile(archive, destination / metadata['entry'])
entry = destination / metadata['entry']
entry.chmod(0o755)
# Agent distributions retain their companion executables, resources and notices.
# Supply only ordinary non-glibc shared runtime libraries under the agent prefix.
libs = destination / 'lib'
libs.mkdir(exist_ok=True)
for name in ['libstdc++.so.6', 'libgcc_s.so.1', 'libatomic.so.1']:
    matches = glob.glob('/usr/lib/*-linux-gnu/' + name) + glob.glob('/lib/*-linux-gnu/' + name)
    if matches:
        shutil.copyfile(matches[0], libs / name)
bin_directory = root / 'usr/local/bin'
bin_directory.mkdir(parents=True)
wrapper = bin_directory / metadata['command']
wrapper.write_text('#!/bin/sh\nset -eu\nexport LD_LIBRARY_PATH="/opt/agents/' + agent + '/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"\nexec "/opt/agents/' + agent + '/' + metadata['entry'] + '" "$@"\n')
wrapper.chmod(0o755)
mod = root / 'opt/mods' / agent
mod.mkdir(parents=True)
manifest = mod / 'mod.json'
paths = sorted(str(path.relative_to(root)) for path in root.rglob('*') if path.is_file() or path.is_symlink())
paths.append(str(manifest.relative_to(root)))
manifest.write_text(json.dumps({'replace': paths}, separators=(',', ':')) + '\n')
# Normalize ownership/permissions for the additive payload; do not ship setuid bits.
for path in root.rglob('*'):
    if not path.is_symlink():
        path.chmod(path.stat().st_mode & 0o777)
