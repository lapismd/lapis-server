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

Production host mounting, named profiles, login/custody, Desktop discovery,
writes, MCP stdio, completions and distribution are still pending. See
[SRV-SRV-010](../../spec/src/current/server.md#srv-srv-010) and the
[content backlog](../../spec/src/backlog/content-automation.md).
