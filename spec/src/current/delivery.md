# Static delivery

Static delivery moved from the Notes hosted-runtime chapter. Shared observation
streams stay in that Notes chapter.

## SRV-DLV-001 — Operator-selected application build

<a id="srv-dlv-001"></a>

**Requirement.** The Rust host MUST select an application build only from an operator-configured absolute directory, independently of HTTP and vault inputs. Startup MUST refuse a missing or invalid application index and any overlap with admitted vaults, the managed profile, operator configuration, or credential files.

### Acceptance details

- `LAPIS_SERVER_STATIC_ROOT` selects the static build directory.
- Overlap with a vault, profile, configuration, or credential file is refused.

## SRV-DLV-002 — Static responses

<a id="srv-dlv-002"></a>

**Requirement.** Static responses MUST stream only regular files beneath the selected build root and reject traversal, hidden paths, malformed encoding, symlinks, and nonregular entries. Public errors MUST omit native paths, and requests MUST NOT modify build output.

### Acceptance details

- GET and HEAD serve only regular files under the selected root.
- Traversal and symlink escapes are rejected without disclosing native paths.

## SRV-DLV-003 — Application fallback

<a id="srv-dlv-003"></a>

**Requirement.** The application index MAY serve valid extensionless navigation paths. Fallback MUST NOT intercept API, plugin resource, missing asset, or non-read requests, and MUST NOT bypass authentication, Origin, or CSRF policy.

### Acceptance details

- Unknown API routes do not return application HTML.
- Missing explicit assets do not fall back to the application index.

## SRV-DLV-004 — Compiled startup

<a id="srv-dlv-004"></a>

**Requirement.** Native hosted startup MUST require explicit operator vault configuration and data-root selection, then launch the Rust executable directly. Missing inputs MUST fail before launch without starting a Node application server or modifying persisted application data.

### Acceptance details

- Startup requires the operator configuration and data root.
- A missing binary or static root fails before launch.
