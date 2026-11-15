# Exact host selection and scoped credentials

Read this before connecting a CLI command to Desktop, a hosted server or an
MCP subprocess. The CLI talks to the selected host's admitted content service;
it is not a direct filesystem vault editor.

## Origins

--target is required for host requests. It must be an exact HTTPS origin, or an
HTTP origin on loopback (localhost or a loopback IP). An origin has no username,
password, query, fragment or non-root path. For example:

```sh
lapis --target http://127.0.0.1:8080 --token-file /protected/lapis-credential --json vault list
```

Use the actual port/origin advertised by the host; 8080 here is only an example.
Do not infer a fallback host from the vault path. A redirect is not followed as a
new trusted origin. Choose one host explicitly for discovery and invocation.

## Credential input

Use exactly one of --token-file and --token-stdin. Do not put the bearer value in
an argv token, URL, note, skill document or printed environment variable.

On Unix, token-file requires a regular owner-owned file with no group/other
permissions and rejects symlinks. A mode such as 0600 meets that policy. This
implementation's token-file flow is Unix-specific. Ordinary commands can consume
a scoped token from stdin where that flow is appropriate; use an existing secret
source rather than echoing a literal token into shell history.

Credentials are bounded, expire and can be revoked. They select admitted vaults
and explicit operations; they do not grant general process or filesystem access.
A read-only credential cannot become a writer by choosing a write command or
loading a skill. Disabled providers and retired vaults remain unavailable even
when local help still describes their command grammar.

## MCP and automation

```sh
lapis --target https://notes.example.com --token-file /protected/lapis-credential mcp serve --stdio
```

MCP stdio reserves stdin/stdout for the protocol, so this mode requires
--token-file. Do not pipe a token into protocol stdin. Keep diagnostics on stderr.
HTTP, MCP and CLI share native operation descriptors and admission; they are not
independent ways around a missing permission. Use the host's supported credential
issuance workflow rather than inventing a CLI login or token-minting command.
