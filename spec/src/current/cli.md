# CLI installation

## SRV-CLI-001 — Per-user command installation

<a id="srv-cli-001"></a>

**Requirement.** The CLI MUST provide local install, status and uninstall commands without host credentials. Installation MUST register `~/.local/bin/lapis`, refuse foreign entries and support idempotent repair. Optional Bash/Zsh PATH setup MUST require explicit consent; removal MUST preserve unrelated shell configuration and user data. Desktop MUST reuse the installation library rather than duplicate it.

### Acceptance details

- Isolated-home subprocess tests execute the installed command and repeat installation and removal.
- Foreign files, directories and links remain unchanged; installer resources and shell paths refuse symlink traversal.
- Only exact marked PATH blocks created by Lapis are removed on explicit request.
- Serialize command and PATH mutations under one per-user lease, including PATH-only cleanup after command removal.

## SRV-CLI-002 — Hosted CLI setup

<a id="srv-cli-002"></a>

**Requirement.** Hosted General → Advanced settings MUST offer versioned CLI downloads and per-user install/removal instructions for the browser computer. Owner-authenticated download metadata MUST be bounded, validate exact release coordinates and checksums, and never forward owner credentials upstream. The UI MUST NOT claim browser-local installation detection or server-side registration.

### Acceptance details

- Validate complete supported-target manifests and refuse altered coordinates, duplicate targets, missing archives and invalid checksums.
- The hosted settings screen offers download/install/removal guidance without reporting browser-local or server registration state.

## SRV-CLI-003 — Bundled server command

<a id="srv-cli-003"></a>

**Requirement.** Server runtime images MUST include an executable `lapis` on PATH for the runtime user. Image acceptance MUST execute CLI help and version without requiring build tools.

### Acceptance details

- Build the server and CLI for each image architecture and copy both executable outputs into the final runtime.
- Runtime-image acceptance checks CLI execution, ownership and read-only installation alongside existing image limits.

## SRV-CLI-004 — Source-bound CLI archives

<a id="srv-cli-004"></a>

**Requirement.** Release automation MUST produce macOS ARM64 and Linux ARM64/x64 CLI archives bound to the exact server release source, target, SHA-256 and size. Identical inputs MUST retain identical archive bytes across retries; publication MUST refuse differing existing assets.

### Acceptance details

- Archive tests unpack standard TAR members and verify exact-byte metadata, reproducibility and source/checksum refusal before upload.
- Release metadata contains every supported target exactly once, and macOS remains build-only under the existing preview limits.

## Executable fixture qualification

SRV-CLI-001 retains every installation, repair, source-switch and tamper refusal
assertion. The source-switch subprocess fixture prepares its alternate executable
in a completed child process. No executable writer descriptor remains in the
multithreaded test runner where concurrent process creation could inherit it.
This harness repair changes no production registration, credentials or lease.

## Approved Desktop connection reuse

The CLI MUST resolve `--target desktop` from the private Unix connection record
`~/.local/state/lapis/desktop.json`. With no target or credential flags, Desktop
is the default. Explicit URL targets retain their explicit credential source;
o remote connection can inherit a Desktop credential. Discovery MUST accept
only version 1, an exact `http://127.0.0.1:<port>` origin and an absolute
credential-file path. Both files MUST pass existing owned/private/regular-file
and no-symlink checks, and record decoding MUST be bounded to 16 KiB.

The record selects an already approved, scoped host credential; no separate
login is required for CLI reads, contributed operations or stdio MCP. Stale,
missing, revoked and expired access MUST fail without host or disk fallback.
MCP stdout MUST remain protocol-only on discovery failure. Help and installation
remain offline. Non-Unix private-file discovery is unsupported.

## CLI release identity

SRV-CLI-004 requires the executable's Cargo version and lock entry to match the
Changesets-owned Server release version. Version preparation synchronizes both;
normal checks and archive CI reject drift before building. A published older
archive remains immutable even when its executable has an obsolete version.
