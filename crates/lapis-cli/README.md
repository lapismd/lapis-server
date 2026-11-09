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

Global flags may appear before or after subcommands. `--json` emits one Protocol
`ContentReply`; human note reads reproduce saved UTF-8 bytes exactly. Human list
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

Production host mounting, plugin settings in Server/Desktop, named profiles,
login/custody, Desktop discovery, writes, completions and distribution remain pending. See
[SRV-SRV-010](../../spec/src/current/server.md#srv-srv-010) and the
[stdio contract](../../spec/src/current/server.md#srv-srv-012), plus the
[content backlog](../../spec/src/backlog/content-automation.md).
