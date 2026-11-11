# OCI runtime Mods

Mods change the live container filesystem, not immutable OCI image layers. They
are operator-trusted executable extensions with the container's initialization
privileges, not sandboxed application plugins. The default image includes jj without Homebrew; selected agent Mods extend
that minimal runtime. Debian/Ubuntu and Alpine acceptance are independent;
no Alpine application compatibility is implied by a portable loader design.

## SRV-MOD-001 — Registry transport and admission

**Requirement.** The standalone `mod-loader` executable MUST load pipe-separated `DOCKER_MODS` references through a maintained OCI client, with verified TLS, configured Docker registry credentials, platform selection for Linux AMD64/ARM64, and digest verification. It MUST reject unsupported schema, platform, compression and multi-layer images. It MUST NOT use a Docker socket or execute environment strings as shell commands.

### Acceptance details

- Run the owning isolated registry, filesystem or lifecycle regression for this boundary.
- Run cold container acceptance on each claimed architecture and base image; retain failures as release gates.

## SRV-MOD-002 — Bounded verified caching

**Requirement.** The loader MUST re-resolve mutable tags, cache artifacts by digest independently of application data, verify cached bytes before use, and serialize shared-cache downloads. Offline startup MUST require an explicitly enabled offline mode and a verified cached resolution. Cache publication MUST be atomic. Authentication and missing-image failures MUST fail without stale-cache fallback during online operation.

### Acceptance details

- Run the owning isolated registry, filesystem or lifecycle regression for this boundary.
- Run cold container acceptance on each claimed architecture and base image; retain failures as release gates.

## SRV-MOD-003 — Safe additive filesystem changes

**Requirement.** The loader MUST validate an entire bounded layer before applying it, reject traversal, escaping symlinks, hardlinks, devices, whiteouts and privileged permission bits, and constrain filesystem operations to the admitted root. It MUST preserve executable modes and numeric ownership. Existing files MUST be preserved unless an exact path is explicitly declared replaceable. Later Mods MAY replace earlier paths only through that declaration.

### Acceptance details

- Run the owning isolated registry, filesystem or lifecycle regression for this boundary.
- Run cold container acceptance on each claimed architecture and base image; retain failures as release gates.

## SRV-MOD-004 — Failure and lifecycle ordering

**Requirement.** Mods MUST apply in configured order before their ordered `init.sh` and `post-init.sh` hooks, original initialization and application execution. Strict mode MUST fail startup on any requested Mod failure; explicitly selected best-effort mode MAY skip failed Mods and MUST log them. File publication MUST be atomic where possible, hooks MUST run only from successfully applied Mod payloads, and initial support MUST NOT add background service supervision.

### Acceptance details

- Run the owning isolated registry, filesystem or lifecycle regression for this boundary.
- Run cold container acceptance on each claimed architecture and base image; retain failures as release gates.

## SRV-MOD-005 — Privilege and execution boundary

**Requirement.** Applying Mods MUST fail explicitly for non-root execution or a read-only target filesystem. The loader MUST NOT escalate privileges. Lapis integration MUST drop supplemental groups and switch to UID/GID 1993 before executing the original entrypoint. With no Mods configured, startup MUST remain compatible with read-only application operation, and the application MUST replace the loader as PID 1, retaining normal signals and graceful shutdown.

### Acceptance details

- Run the owning isolated registry, filesystem or lifecycle regression for this boundary.
- Run cold container acceptance on each claimed architecture and base image; retain failures as release gates.

## SRV-MOD-006 — Receipts and diagnostics

**Requirement.** Successful application MUST record references, resolved manifest/layer digests, platform and installed paths in container-local receipts. Structured logs MUST distinguish resolution, cache, apply, hooks and failures without printing credentials. Mod cache and receipts MUST reject unsafe ownership, symlinks and world-writable state.

### Acceptance details

- Run the owning isolated registry, filesystem or lifecycle regression for this boundary.
- Run cold container acceptance on each claimed architecture and base image; retain failures as release gates.

## SRV-MOD-007 — Reusable distribution and qualification

**Requirement.** Reusable scratch-image templates, executable and initialization-hook examples, Compose configuration, GHCR publication and custom-image integration instructions MUST accompany the loader. CI MUST exercise registry authentication, missing images, platform mismatch, corrupt blobs, malicious archives, cache reuse/update, strict/best-effort failures, restart/recreation and original-entrypoint signal behavior. Agent runtimes MUST be separately selectable Mods without Homebrew runtime dependencies.

### Acceptance details

- Run the owning isolated registry, filesystem or lifecycle regression for this boundary.
- Run cold container acceptance on each claimed architecture and base image; retain failures as release gates.

- Publish catalog-derived CLI-version, version-plus-commit and commit tags for the same already qualified multiarchitecture index.
- Document root/writable initialization and tag/digest selection in a runnable Mod Compose example.
