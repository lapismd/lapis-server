# Shared OCI Mod loader

`ghcr.io/lapismd/mod-loader:<commit>` is an independently built, static Linux
loader image. It exports `/usr/local/bin/mod-loader` for both AMD64 and ARM64.
Use its immutable multi-platform digest in production:

```dockerfile
FROM ghcr.io/lapismd/mod-loader:<qualified-version> AS mod-loader
FROM alpine:3.23
RUN apk add --no-cache ca-certificates
COPY --from=mod-loader /usr/local/bin/mod-loader /usr/local/bin/mod-loader
# Keep the original initialization program, arguments and CMD.
ENTRYPOINT ["/usr/local/bin/mod-loader", "--", "/original-entrypoint"]
```

The executable is independent of Lapis, s6, Homebrew, the base image's dynamic
libc, and package managers. Registry TLS uses the base's CA trust store; install
`ca-certificates` or mount a trusted CA bundle. Mod payloads have their own ABI
requirements. In particular the Cursor/Claude/OpenCode agent Mods are qualified
on Ubuntu, not implied compatible with Alpine. No Docker socket is used.

The library/executable is a standalone Rust workspace under
`crates/lapis-container-mods`, with its own locked dependencies. Its artifact
can be reused by other projects without copying initialization logic. The
Container Mods workflow compiles and qualifies native AMD64 and ARM64 once,
then publishes those exact tested binaries as a scratch OCI image. Lapis copies
that shared artifact into its smaller runtime.

## Configuration

| Variable                          | Default                     | Meaning                                                                       |
| --------------------------------- | --------------------------- | ----------------------------------------------------------------------------- |
| `DOCKER_MODS`                     | empty                       | OCI image references separated by `\|`, applied left to right                 |
| `DOCKER_MODS_CACHE`               | `/var/cache/container-mods` | Separate, root-owned cache; use its own named volume                          |
| `DOCKER_MODS_STRICT`              | `true`                      | Fail startup for a requested Mod or hook failure                              |
| `DOCKER_MODS_DEBUG`               | `false`                     | Additional diagnostics, without transport/credential details                  |
| `DOCKER_MODS_OFFLINE`             | `false`                     | Explicitly use only previously resolved, digest-verified cache entries        |
| `DOCKER_MODS_INSECURE_REGISTRIES` | empty                       | Explicit comma-separated HTTP test registries; production uses verified HTTPS |

Configure private registries by mounting a Docker `config.json` secret and
setting `DOCKER_CONFIG` to its directory. Standard `auths`, identity tokens and
credential helpers are supported; a configured helper must be available in the
container. Credential files must be owned by the initialization user and not
group/world writable; configured helpers and their entire PATH must likewise
be protected from application writes. Use a root-owned Docker config secret
instead of an app-writable profile directory. No credentials, provider secrets
or tokens are embedded in Mods.
Mutable tags are resolved at every online start; missing/authentication errors
never cause an implicit stale-cache fallback. Offline mode is explicit and
requires a complete verified resolution and blobs. A shared cache is serialized
with a filesystem lock; temporary files are atomically renamed after validation.

## Filesystem and hooks

A Mod is exactly one raw-tar or gzip-tar OCI filesystem layer, ordinarily built
with `FROM scratch` and `COPY root/ /`. Only Linux AMD64/ARM64 image config is
admitted. Other layer counts, compression, devices, hardlinks, whiteouts,
setuid/setgid/sticky files and extended GNU/PAX archive records are explicitly
unsupported. Limits are 1 GiB compressed, 4 GiB expanded and 100,000 entries.

The full layer is staged under generated filenames and validated before target
writes. Archive paths must be relative and cannot traverse upwards. Symlinks
must be relative and remain within the admitted root; entries cannot write
through symlink parents. Capability-based filesystem access prevents escapes.
Directory merges preserve existing directory ownership/modes. New files retain
executable modes and numeric ownership. Existing files cause failure by default.
For deliberate replacements, add `opt/mods/ID/mod.json`:

```json
{ "replace": ["usr/local/bin/my-tool", "etc/myapp/example.conf"] }
```

Only exact paths are accepted. A later Mod wins a collision only with explicit
replacement intent. Base files are never silently deleted. Per-file publication
uses temporary files and atomic rename, with a rollback journal for replaced
files when application fails; newly created empty parent directories may remain.
A receipt is stored in `/var/lib/container-mods/applied.json` inside each
container, separately from application data and the download cache. An unchanged
receipt avoids reapplication only when installed files still match the payload.

After all file layers have applied, executable regular files at
`opt/mods/ID/init.sh` run in configured Mod order (IDs sort within a Mod), followed
by all `post-init.sh` hooks in that same order. Hooks are explicit package/setup
operations; file names do not implicitly invoke a package manager. A hook failure
blocks strict startup. Best effort logs failures and continues; hook side effects
cannot be rolled back. No background services are added or supervised.

Mods are trusted code with initialization privileges. Requested Mods require a
root initialization process and writable container filesystem, and fail explicitly
otherwise; the loader never escalates privileges. Use `--uid 1993 --gid 1993` to
drop supplemental groups and identity before the original application entrypoint.
The loader exits promptly on initialization SIGTERM/SIGINT and then calls `exec`,
so the application retains PID 1 and receives signals after startup.
Without Mods, non-root/read-only startup remains supported and no cache is opened.
Files change the running container's writable layer, not the original OCI image.
Recreated containers apply verified cached payloads to their own new filesystem.

## Examples and local testing

`docker/mods/template/Dockerfile` is the reusable scratch template. The `tool`
example installs `mod-example-tool`; the `hook` example writes a marker before
the original application starts. `docker/mods/compose.example.yml` shows Lapis
with a separate Mod cache. Set `DOCKER_MODS` to a pipe-separated selection of
versioned or digest-pinned agent Mods: `ghcr.io/lapismd/mods:codex-<commit>`,
`ghcr.io/lapismd/mods:cursor-<commit>`, `ghcr.io/lapismd/mods:claude-<commit>`
and `ghcr.io/lapismd/mods:opencode-<commit>`. Each also has an
`<agent>-<upstream-version>-<commit>` tag. Each native architecture test injects
the complete hash-pinned upstream distribution and runs its version command as
the application user before those exact artifacts are published.

```sh
docker --context omarchy build -f docker/mod-loader/Dockerfile -t lapis-mod-loader:qualification .
LAPIS_DOCKER_CONTEXT=omarchy sh scripts/test-container-mods.sh
cargo test --manifest-path crates/lapis-container-mods/Cargo.toml --locked
```

For a custom Mod, create `root/usr/local/bin/your-tool`, preserve executable
permissions, optionally add `root/opt/mods/ID/init.sh`, then build/push the scratch
template using normal Docker Buildx platform flags. Pin its resulting manifest
digest in `DOCKER_MODS`. Publish only payloads compatible with the target base.
