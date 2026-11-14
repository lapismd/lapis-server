---
name: lapis-cli
description: Use the Lapis CLI to discover admitted vaults, read notes, inspect host capabilities and invoke contributed plugin commands.
---

# Lapis CLI

Run `lapis --help` and `lapis help` before choosing commands. Help and plugin
operations reflect the installed CLI and selected host. This is a Lapis client;
Obsidian CLI commands and `key=value` arguments do not apply.

Select the exact host with `--target https://your-lapis-host` (HTTP is allowed
only for loopback). Supply scoped credentials through `--token-file` or
`--token-stdin`, never a token argument, URL or environment printed in output.
Keep credentials out of notes, skill files and shell history. Use `--json` when
parsing results. Follow error exit codes and structured outcomes.

```sh
lapis --target https://your-lapis-host --token-file /protected/credential capabilities
lapis --target https://your-lapis-host --token-file /protected/credential vault list
lapis --target https://your-lapis-host --token-file /protected/credential note list --vault <vault-id>
lapis --target https://your-lapis-host --token-file /protected/credential note read --vault <vault-id> --path "Projects/Plan.md"
```

Use the admitted vault ID returned by discovery. Paths are vault-relative and
must stay within the selected vault. Do not infer a vault from the working
directory or a similarly named note. Quote shell arguments and pass note text
as data. Check capabilities and contributed help before attempting a mutation;
a read-only credential cannot acquire write authority by choosing a command.

Plugin commands use `lapis <namespace> <operation> --vault <vault-id>`.
Inspect `lapis help <namespace> <operation>` for fields, JSON arguments and
examples. Installed plugins and permission grants determine availability.

```sh
lapis --target https://your-lapis-host --token-file /protected/credential skills list --vault <vault-id>
lapis --target https://your-lapis-host --token-file /protected/credential skills install --vault <vault-id> --skill lapis-markdown
lapis --target https://your-lapis-host --token-file /protected/credential skills update --vault <vault-id>
```

Skill updates preserve edited and unmanaged directories. Inspect skipped names
and reasons; do not replace those files manually as an automatic workaround.
Plugin upgrades supply new skill content; the skills command does not upgrade
plugins. Never automatically replay a write after an uncertain transport result.
Inspect the original request/invocation ID before deciding whether to retry.
