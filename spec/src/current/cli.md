# CLI installation

## SRV-CLI-001 — Per-user command installation

<a id="srv-cli-001"></a>

**Requirement.** The CLI MUST provide local install, status and uninstall commands without host credentials. Installation MUST register `~/.local/bin/lapis`, refuse foreign entries and support idempotent repair. Optional Bash/Zsh PATH setup MUST require explicit consent; removal MUST preserve unrelated shell configuration and user data. Desktop MUST reuse the installation library rather than duplicate it.

### Acceptance details

- Isolated-home subprocess tests execute the installed command and repeat installation and removal.
- Foreign files, directories and links remain unchanged; installer resources and shell paths refuse symlink traversal.
- Only exact marked PATH blocks created by Lapis are removed on explicit request.

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
