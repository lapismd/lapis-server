# Lapis CLI commands and discovery

Read this for command selection, output handling and contributed operation help.
This is Lapis's content-host client, not the Obsidian CLI. Commands and flags
are conventional argv tokens, not key=value fields.

## Local grammar and host discovery

```sh
lapis --help
lapis --version
lapis help
lapis help skills install
lapis help -k skills
```

Bundled operation help can be available offline. It describes the command
contract, not permission to run it against every host. Discover live availability
using the exact selected host and credential:

```sh
lapis --target https://notes.example.com --token-file /protected/lapis-credential --json capabilities
lapis --target https://notes.example.com --token-file /protected/lapis-credential --json vault list --limit 50
```

Choose the admitted vault UUID from discovery. Do not infer a vault from the
working directory or a similarly named directory. In the examples below,
vault_id and cursor mean values obtained from the selected host, not credentials.

```sh
lapis --target https://notes.example.com --token-file /protected/lapis-credential --json note list --vault "$vault_id" --directory "Projects" --limit 100
lapis --target https://notes.example.com --token-file /protected/lapis-credential --json note list --vault "$vault_id" --limit 100 --cursor "$cursor"
lapis --target https://notes.example.com --token-file /protected/lapis-credential note read --vault "$vault_id" --path "Projects/Plan.md"
```

List limits are 1–100, default 50. Follow an opaque next_cursor to obtain later
pages; do not manufacture it. Ordinary note read writes the saved note text to
stdout. --json writes the stable reply envelope; inspect outcome and its value
rather than parsing human-readable rows. Diagnostics and continuation hints use
stderr. A dirty renderer buffer is not necessarily the same as the saved note.

## Plugin operations

The shared operation registry supplies namespaced operations, typed inputs,
permission requirements, help and examples. Use lapis help <namespace> <operation>
to inspect the installed contract; invocation rechecks current host admission.
Put --target, credential and --json flags before contributed commands. Treat JSON
arguments/input files as data and use the advertised input format, not shell eval.

Read [skill management](skill-management.md) for the packaged skills operations.
Do not invent Obsidian commands or assume a newly installed plugin grants an
existing credential more permissions.

## Error outcomes

| Exit | Meaning |
| --- | --- |
| 0 | Command completed successfully |
| 2 | Invalid request/arguments |
| 3 | Missing, invalid or expired credential |
| 4 | Conflict or already exists; refresh state first |
| 5 | Not found |
| 6 | Permission denied |
| 7 | Busy or unavailable |
| 8 | Protocol mismatch |
| 9 | Size limit exceeded |
| 10 | Unsupported operation or credential flow |
| 11 | Outcome unknown |

A successful command can still return per-skill skipped results. Inspect that
result separately from process failure. Never automatically replay a mutation
whose outcome is unknown. Use the returned request/invocation identity and the
host's advertised inspection facilities; do not invent a retry/status command.
