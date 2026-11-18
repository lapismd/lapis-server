# Preserve normal managed-note consumption

SRV-GOV-001/003 keep the source-owned schema2 contract for15 existing Deno-free
root inputs. This source follow-up changes the note-loading lifecycle only;
manifest,workspace,full lock,source policy,version recipe/configuration,private
false release intent and existing production HIGH99 are unchanged. No bot branch
is edited, no audit exception is granted and no version/package/image is published.

## Exact original failure

At immutable source `b49ac751c595630e976d9a27f2c1ee2135a9ce9f`, an isolated real
Jujutsu baseline contains the actual coordinator-shaped empty managed note
`.changeset/lapis-dependencies-6882649476db.md`. Installed Changesets3.0.3
`version` consumes that note without changing dependency facts. The production
wrapper with its default public-SDK VCS reader then fails ENOENT while resolving
the deleted current path. The original cache receipt retains exact CLI stdout,
changed path and failure; the same directory/default wrapper passes after repair.
This is real CLI note consumption, not a fabricated version or release.

The first test harness omitted the note directory in independent unsafe-file
fixtures (8failures). After fixing fixture setup, the coherent original target
has4failures/10passes: legitimate Git and JJ consumption both failENOENT, the
missing-note refusal fails at the wrong filesystem boundary, and an unchanged-
facts oversized live note is incorrectly accepted. These failures remain in logs.

## Owning repair and controls

The loader uses `lstatSync(...,{throwIfNoEntry:false})` and skips only undefined.
It retains every changed path. Current files retain realpath/regular-file checks,
with the8192-byte stat bound checked before reading. Dangling/external links,
directories,malformed or nonempty live notes and ordinary I/O failures are not
consumption. Server's fixed private owner permits only empty managed release
maps, so present notes use the existing source parser even on unchanged facts.
Actual dependency change with a changed-but-deleted managed note still fails the
existing missing-bytes parser. Normal human release notes remain separately
governed by unchanged Changesets configuration.

An internal optional source-reader factory is solely a test port; the production
CLI uses the original strict reader. Committed tests exercise the actual public
SDK Git/JJ unified-diff parser against the same immutable baseline and complete
fixture input changes. They invoke real Changesets version consumption, assert
the note is gone and all three manifest/workspace/lock bytes stay identical.
No Git-write or installed JJ dependency is added to Git-only source CI.

All15 lifecycle/safety cases pass: Git/JJ consumption, actual published Core
patch with live empty note, deleted changed-note refusal, directory, live/dangling/
external symlinks,oversized/malformed notes,nonempty/null maps,external changed
paths and retained public-owner required-release-map refusal. All85 intake cases
pass. The normal production-reader original reproduction passes after the fix.

## Source qualification

Node24.15.0/pnpm10.34.5/Changesets3.0.3/Node-only SDK0.2.1 remain pinned. QMD
semantic discovery returned owning automation/toolchain chapters; doctor reports
15passes and the intentional portable alias warning. Root check passes with
0Svelte errors/warnings,App ownership and4release-auth cases. Root test passes
85intake cases,8CLI/auth cases and218unit cases across37files. Renderer build
passes with its visible existing chunk-size warning. The complete owning spec,
formatter,mapped source gate and immutable-input comparison are qualified before
push; this validation adds no Rust or UI lifecycle change. Native/hosted/image
gates and original issue99 remain independent and required.

## Read-only same-pattern audit

AI `52a8236d` has the same unconditional current-file read at
`check-canonical-intake.mjs:271–272`; its maintenance verifier also runs before
unchanged-facts return at188, so a loader-only fix is insufficient. Root owns
AI17/its follow-up; this task edits no AI source.

Core `e9985eae`,Mira `88d1d214` and Plugins `87154adb` use existsSync-gated
loads at `check-workspace-intake.mjs:83–85`, followed by unchanged-facts return.
Ordinary consumption therefore avoids ENOENT; the latent dangling-link-as-absent
weakness of existsSync remains an owning safety follow-up, not silently repaired.
Docs `605a731f` and Tasks `c2601039` have no managed-note file loader in their
single-development-Core profile; they have no same deleted-note read failure.
Their unchanged-facts return and actual dependency scope gates remain distinct.
No other source trees are modified, no PR checks are polled/watched and no
workflow/version/audit permissions are changed by this source slice.
