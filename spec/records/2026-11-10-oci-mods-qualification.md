# OCI Mods source and remote container qualification

The standalone loader has its own locked Rust workspace. Its 17 cases pass:
eight filesystem/configuration/credential cases and nine registry/cache/lifecycle
cases. All-target Clippy denies warnings. Coverage includes standard private
registry authentication, digest-pinned multi-platform resolution, missing images,
architecture mismatch, multiple layers, corrupt downloaded/cached bytes, atomic
cache publication, shared-cache locking, strict/best-effort failures, hooks,
ordered replacements across restart, path/symlink admission and ownership.
Additional regressions cover rollback bytes/permissions, root-level symbolic
links, chained links and traversal through existing/archived link graphs.

All container testing after the user's context change uses `omarchy`, the
remote Linux x86_64 engine. Existing unrelated containers and volumes are
preserved; test cleanup removes only task-named resources. A fresh static-musl
artifact builds successfully and is exported from a scratch image. Both direct
multi-stage COPY and a named OCI-layout build context use that artifact.
The ELF has no interpreter dependency; it is approximately 9 MB unpacked and
approximately 3.7 MB as a compressed OCI artifact. Base CA trust is required.

Cold Alpine 3.23 and Ubuntu 24.04 container acceptance passes an actual OCI
registry executable Mod plus initialization/post-initialization hooks, multiple
Mods, download/cache reuse, explicit offline operation, fresh container
recreation, same-container restart and original-entrypoint SIGTERM handling.
Initialization SIGTERM also exits promptly with code 143 during a waiting hook,
without starting the application. No-Mod read-only/non-root operation succeeds. Requested Mods reject unsupported
non-root/read-only operation clearly. A separate real Docker Hub hello-world
single-layer image is resolved, verified, installed and executed without a
Docker socket. No background supervisor is added.

The four pinned AMD64 upstream distributions install as separate scratch Mods
and execute their version commands as UID/GID 1993 without Homebrew: Codex
0.167.0, Cursor 2026.11.05-86047df, Claude Code 2.1.311, OpenCode 2.2.6.
Agent tests use an explicitly writable test HOME after privilege drop; the
application's existing HOME is preserved. Payload checksums are in agents.json.
An initial upstream HTTP 403 from the Python default User-Agent was corrected
by using the named builder User-Agent; downloaded bytes still require the pin.

A remote Lapis packaging-only image reuses the public 45f4c1b AMD64 executable
and UI with the new minimal runtime/loader. Real readiness, original read-only
startup, injected executable/hook visibility, PID-1 UID 1993 and graceful stop
pass. Its unpacked layers total about 252 MB, within the 500 MiB base budget.
Docker 29 containerd inspect Size reports compressed storage, so the acceptance
budget sums raw history layer bytes instead. This separates it from the earlier
ARM64 packaging receipt and avoids comparing unlike size metrics.

Specification/ownership checks, formatting, shell syntax and actionlint pass.
Native ARM64 loader/agent acceptance, fresh source-built Server image checks,
hosted PR CI and anonymous public artifact verification remain release gates.
The package-startup fix has a separate Notes release and Server dependency
refresh path; this packaging receipt does not claim it is in the public image.

Initial hosted CI failed before starting jobs because its reusable workflow
requested package read access absent from the caller. The caller now explicitly
grants read-only package access; publication remains separate.

Both native hosted lifecycle jobs then exposed a test readiness race: the test
stopped the restarted container before the loader had execed the application,
then expected application SIGTERM behavior. The test now waits for a second
application readiness marker after restart, emitted after installing its signal
handler. Initialization SIGTERM remains separately asserted as exit 143. The
unchanged target passed on remote AMD64 before the fix; its slower remote Docker
round trips had masked the hosted timing difference.
