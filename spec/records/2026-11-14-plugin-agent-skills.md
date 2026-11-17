# Native packaged skills and CLI qualification

Core owns discovery, the protected installer and the built-in `lapis-cli`
skill. The operation provider exposes list/read authority separately from
`skills:write`; existing read-only grants acquire no mutation authority.
Enabled admitted plugins contribute verified Markdown/text assets without
executing renderer code. Hosted web invokes the same admitted operation client.

The authenticated command surface is:

```sh
lapis skills list --vault <id>
lapis skills install --vault <id> --skill lapis-markdown
lapis skills update --vault <id>
```

Existing target, credential and JSON options apply. Commands never upgrade
plugins or fetch upstream content. Native writes hold the vault writer lock and
editor guard. Durable pending receipts distinguish incomplete operations from
completed updates, including after a cold reopen.

## Source and acceptance checks

- Core's full default suite passes 579 cases with seven existing ignores.
  The final focused packaged-skill suite passes seven cases, including a later
  revoked-source regression. Coverage includes fresh/repeated installation,
  clean upgrades, obsolete files, local edits/additions/deletions, unmanaged
  collisions, corrupt/pending receipts, unsafe paths/symlinks, source conflicts,
  disabled/revoked sources and scoped credential failures.
- The disabled bundled-source test failed once before filtering enabled state
  independently of the installed registry; the exact target passes afterward.
- Full CLI suite passes 18 cases. The real HTTP/CLI regression initially failed
  once because stdin credentials were consumed separately for catalog and
  invocation. Reading credentials once fixes the owning CLI transport.
  The test installs through native App services, cold-starts CLI processes
  against the same isolated vault and proves user edits remain protected.
- Full native Server host suite passes 146 cases; HTTP/MCP transports pass 22.
  Strict all-target Clippy passes for Core, CLI and Server.
- Hosted adapter regression passes one case. Web's full unit suite passes
  131 cases; type checks and production build pass. It retains session identity,
  selected skill and uncertain-write behavior without renderer fallback.
- Two consumer targets initially failed against a stale Markdown archive with
  a relative JSON import. The owner corrected its public package import;
  refreshed immutable development archives pass both exact targets (eight
  cases), the full web suite and build. Registry manifests/lockfiles are retained.
- Spec-validator 0.2.0 doctor/QMD discovery and specification gates pass.
- Full cold hosted Storybook interaction/accessibility qualification passes
  58/58. Its initial four failures used the credential button name from before
  the existing scoped-operation UI; the exact interaction passes 4/4 with the
  current scoped-credential contract, followed by the full catalog.

Docker stages only the statically bundled Markdown contribution through its
public package export; dynamically admitted Bases and Canvas use verified
installed assets. Publication and release-version intake remain separate gates.
