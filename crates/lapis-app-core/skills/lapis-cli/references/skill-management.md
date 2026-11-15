# Packaged skill installation and protected updates

Read this for explicit installation, receipt ownership and skipped results.
The selected host contributes its CLI skill plus eligible enabled plugin skills.
Discovery uses admitted verified package assets, not renderer execution or an
upstream GitHub download.

```sh
lapis --target https://notes.example.com --token-file /protected/lapis-credential --json skills list --vault "$vault_id"
lapis --target https://notes.example.com --token-file /protected/lapis-credential --json skills install --vault "$vault_id" --skill lapis-markdown
lapis --target https://notes.example.com --token-file /protected/lapis-credential --json skills update --vault "$vault_id"
```

Omit --skill to operate on all eligible contributions. Select by contributed
skill name (lapis-markdown, lapis-bases, lapis-json-canvas or lapis-cli), not by
plugin ID or directory name. Availability depends on the host and enabled plugins.

## Meaning of each action

- list observes available/managed skill state and requires skills:read.
- install adds absent directories; an existing managed older version is left for
  update. Existing unmanaged directories are preserved.
- update refreshes already managed, completely unchanged directories and requires
  skills:write, as does install. Clean updates remove obsolete packaged files.

The destination is <vault>/.agents/skills/<name>/. The versioned receipt is
<vault>/.agents/.lapis-skills.json, containing source identity/version, SHA-256
hashes and pending state. Do not manually edit receipts to claim ownership of
existing files. New packaged references are installed with the skill and are
protected by the same whole-directory comparison as SKILL.md.

## Results and recovery decisions

Installed, updated and unchanged are distinct successful states. Available is
an observation from list. Skipped results include a name and reason:

| Reason | Meaning |
| --- | --- |
| unmanaged | Existing directory has no valid managed ownership |
| locally-modified | A file was changed, added or deleted, including a reference |
| owner-changed | The packaged source identity differs from the receipt |
| interrupted | A prior write remains pending |
| not-installed | Update was requested for an unmanaged/uninstalled skill |
| update-available | Install found a managed version that needs explicit update |

Operational failure is separate from a per-skill skipped result. Invalid receipts,
conflicting names, traversal and symlink escapes fail closed. An uncertain write
outcome must be inspected before retrying. There is no force-overwrite flag.
If local instructions matter, compare the packaged version and merge deliberately;
do not replace the directory as an automated workaround for a skipped update.

Plugin upgrades deliver newer packaged content; skills install/update do not
upgrade plugins or fetch upstream skill repositories. Disabling/uninstalling a
plugin does not authorize deleting an installed skill. Changes affect future
agent bindings or an explicit skill refresh; existing immutable agent snapshots
retain their captured instructions.
