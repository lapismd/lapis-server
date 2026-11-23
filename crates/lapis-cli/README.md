# Lapis CLI preview

Build with `cargo build -p lapis-cli --locked`, or use `cargo run --locked -p
lapis-cli -- --help`. This source preview reads an explicitly selected content
HTTP host; it never opens a vault as another native owner.

```sh
lapis --target https://notes.example --token-file "$LAPIS_TOKEN_FILE" capabilities
lapis --target https://notes.example --token-file "$LAPIS_TOKEN_FILE" vault list --json
lapis --target https://notes.example --token-file "$LAPIS_TOKEN_FILE" note list --vault <UUID> --limit 25 --json
lapis --target https://notes.example --token-file "$LAPIS_TOKEN_FILE" note read --vault <UUID> --path Inbox/note.md
```

Use `--token-stdin` instead of `--token-file` to pipe a credential from its owner
or credential manager. There is no token argument. Unix token files must be
regular files owned by the effective user without group/other permissions; final
symlinks and FIFOs are refused. Other platforms currently use stdin. The host
must issue the scoped credential; owner-wide server tokens do not grant access.

Global flags may appear before or after built-in subcommands. Host/credential
flags precede plugin commands. `--json` emits one Protocol reply; human note reads
reproduce saved UTF-8 bytes exactly. Human list
output uses tab-separated columns, with next-page hints on stderr. Lists accept
`--cursor`, and note lists accept `--directory`. Vaults use explicit UUIDs.
Errors use stderr and, in JSON mode, the structured stdout reply.

The target must be an HTTPS origin, or HTTP on loopback. Redirects, environment
proxies, URL credentials/query/path suffixes and local-vault fallbacks are
disabled. `--timeout-ms` bounds a request to 1–120000 ms (default 15000).

Exit codes: 0 success; 2 invalid input; 3 unauthenticated; 4 conflict; 5 not found;
6 permission denied; 7 busy/unavailable; 8 protocol mismatch; 9 too large;
10 unsupported; 11 unknown effect. Parser syntax errors use help/stderr and 2.

For a subprocess MCP client on Unix, use:

```sh
lapis --target https://notes.example --token-file "$LAPIS_TOKEN_FILE" mcp serve --stdio
```

This connects to the host's `/mcp` endpoint. Stdin/stdout carry only MCP frames;
`--token-stdin` and `--json` are invalid in this mode. It relays the host's
tool/resource catalog and scoped reads, without a local vault runtime. The
qualified SDK is `rmcp 3.9.0`: upstream HTTP uses `2026-07-28` discovery, while
stdio clients can use that revision or `2025-11-25` initialization. Public MCP
client products and non-Unix credential custody are not yet qualified.

Upstream requests use the same explicit target rules and timeout flag, no
redirects/proxies/retries, and no response cache. Calls forward once; interactive
continuations are unsupported. There are at most 64 active forwards. Stdio input
lines are limited to 64 KiB; an oversized line is discarded and the following
request can proceed. EOF or a failed stdout closes the bridge and drains its
async services without hanging on a blocking stdin reader. Transport diagnostics
are sanitized and use stderr.

The hosted server now mounts these endpoints when enabled in **Settings → Core plugins
→ Content API and MCP**. Create a scoped credential there, copy it into your
protected token file, and use the public origin shown in settings. Credentials
expire within 24 hours and never survive server restart. Desktop mounting and
plugin settings, named profiles, login/custody, Desktop discovery, general note writes,
completions and distribution remain pending. See
[SRV-SRV-010](../../spec/src/current/content.md#srv-srv-010) and the
[stdio contract](../../spec/src/current/content.md#srv-srv-012), plus the
[content backlog](../../spec/src/backlog/content-automation.md).

Compiled plugin commands use live operation discovery and the same admitted
native services as HTTP/MCP. Tasks includes 17 operations and bundled offline
Markdown help: `lapis tasks complete --help`, `lapis help tasks` and
`lapis help -k tasks`. Use `--input-file` for a JSON input object (64 KiB maximum)
and `--invocation-id` to retain an explicit lifecycle identity. See
[plugin operations](../../docs/plugin-operations.md) for scopes and recovery.

## Install the command

Official archives contain a standalone executable for macOS ARM64 and Linux
ARM64/x64. Verify the archive SHA-256 before extraction. Run `./lapis cli install`
to install a private copy at `~/.local/share/lapis/cli/lapis` and register
`~/.local/bin/lapis`. No server or credential is required for installation.
Use `--setup-path bash` or `--setup-path zsh` only if you want marked shell PATH
entries. Other shells require manual PATH setup. Open a new terminal afterward.
`lapis cli status` reports registration; `lapis cli uninstall --remove-path`
removes the owned command and unchanged marked PATH entries, retaining vaults
and credentials. Conflicting files/links are never replaced.

A private installation receipt binds the registered copy to its original source
and SHA-256. `--replace` explicitly switches between Lapis-owned sources;
unrelated or modified binaries are still refused. A missing owned registration
can be repaired by repeating installation. Setup mutations use an exclusive
per-user lock.

### Connect to an open Desktop

Enable local access in Desktop's General → Advanced settings, then create a
credential for the desired vaults, permissions and lifetime. Save its private
file outside your vault. On Unix, Desktop remembers that approved connection
locally; the CLI and MCP reuse it without a separate sign-in:

```sh
lapis vault list --json
lapis --target desktop capabilities
lapis --target desktop mcp serve --stdio
```

An MCP subprocess configuration can use command `lapis` with arguments
`["--target", "desktop", "mcp", "serve", "--stdio"]`. The command must be a
build containing Desktop discovery. An older Desktop-bundled CLI still requires
explicit `--target <origin> --token-file <file>` arguments.

Access expires or stops working after revocation, vault closure, disabling
access or Desktop restart. Recreate the credential in Desktop to reconnect.
Remote HTTPS targets still require an explicit credential; local credentials
are never borrowed for a remote target. Windows discovery and durable approvals
across restarts remain unqualified.
