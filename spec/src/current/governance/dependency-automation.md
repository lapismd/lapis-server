# Dependency automation

SRV-GOV-001/003 govern the shared coordinator integration. Its source-owned schema2 profile admits compatible published patch updates
for the existing root first-party registry dependencies, including Design Core
and the Node-only specification validator. Cargo/git/native/private archive
migrations, dependency additions, peer changes, major and 0.x minor updates
retain source review. Independent normal checks validate the complete
manifest/workspace/lock closure and two factual slots outside trusted-bot
specification exemptions. Mixed, stale, manual or unsupported updates fail closed.

The completion observer loads only the same reviewed pinned shared implementation
and policy, verifies the exact source workflow and current revision before App
minting, and executes no candidate code or artifacts. Publication remains Server
owned; registration and callbacks do not activate a repository.

One exact managed dependency Changeset may accompany a compatible patch only
when its YAML release map is empty. Nonempty, malformed, multiple or arbitrary
notes remain rejected; no product version intent is added.

Normal version consumption MAY delete a managed note when dependency facts are
unchanged. The loader MUST skip only a truly absent current file, retaining its
changed path so actual dependency intake still rejects missing note bytes.
Existing files MUST retain regular-file/realpath,8192-byte and empty-map checks;
dangling or external symlinks, directories, malformed notes and I/O errors MUST
not be treated as consumed notes.

Git and Jujutsu changed paths MUST use the same captured immutable parent as
prior input facts. Clean hosted Git checkouts MUST retain tracked managed-note
and unexpected-file scrutiny; one managed note stays bounded to8192 UTF-8 bytes
with an empty release map. This changes no source version or audit policy.

The private root carries no automatic product-release intent under the existing
coordinator policy. Core runtime updates therefore retain an empty managed note;
a genuine manually authored Server release remains governed by its unchanged
private-package Changesets configuration. No npm package is published. Registry
transitives must pass native frozen installation and all source audits/product
gates; unchanged first-party identities, metadata, peers and direct edges remain
immutable. Unavailable Profile/Tasks publication waits and production HIGH issue99
are not waived by this source capability.

<!-- dependency-intake:server-registry:start -->

Current registry dependency coordinates; qualification remains separate.

<!-- prettier-ignore -->
| Manifest | Dependency | Range | Locked version |
| --- | --- | --- | --- |
| `package.json` (dependencies) | `@lapis-notes/api` | `^0.6.5` | `0.6.5` |
| `package.json` (dependencies) | `@lapis-notes/app-profile` | `^0.2.2` | `0.2.2` |
| `package.json` (dependencies) | `@lapis-notes/file-explorer` | `^0.2.8` | `0.2.8` |
| `package.json` (dependencies) | `@lapis-notes/history` | `^0.5.1` | `0.5.1` |
| `package.json` (dependencies) | `@lapis-notes/language-service` | `^0.1.6` | `0.1.6` |
| `package.json` (dependencies) | `@lapis-notes/markdown` | `^0.3.3` | `0.3.3` |
| `package.json` (dependencies) | `@lapis-notes/search` | `^0.1.12` | `0.1.12` |
| `package.json` (dependencies) | `@lapis-notes/source-editor` | `^0.1.11` | `0.1.11` |
| `package.json` (dependencies) | `@lapis-notes/ui` | `^0.2.1` | `0.2.1` |
| `package.json` (dependencies) | `@lapis-notes/workspace` | `^0.4.6` | `0.4.6` |
| `package.json` (dependencies) | `@lapismd/design-core` | `^0.13.4` | `0.13.4` |
| `package.json` (dependencies) | `@lapismd/jj-history` | `~0.1.0` | `0.1.0` |
| `package.json` (dependencies) | `@lapismd/mira` | `~0.2.0` | `0.2.0` |
| `package.json` (devDependencies) | `@lapis-notes/lapis-plugin-tasks` | `^0.1.1` | `0.1.1` |
| `package.json` (devDependencies) | `@lapismd/spec-validator` | `0.2.1` | `0.2.1` |

Manifest, peer and integrity facts SHA256: `917dfb007485824f24c0b11ce068aa13cb7fb9363e7ffbbfddd58c3be331ff42`.

<!-- dependency-intake:server-registry:end -->
