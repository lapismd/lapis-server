# Development intake source range and note bounds

This is a Server-owned Node tooling guard repair on current main
`648222c00549b8737a837c0e622eb7e360828ee0`, preserving the landed #101 hosted
failure-diagnostics repair. It changes no application, Rust/model, publication,
manifest/lockfile, version recipe, source-policy or advisory inputs.

## Before evidence

The independent prior-file reader selects Git `HEAD^1`, but its wrapper called
`changesFromVcs({})`, whose clean-Git working-tree default returns no paths.
The committed released SDK 0.2.0 to 0.2.1 fixture reproduces four false acceptances:
runtime and null managed notes and an unexpected implementation file disappear
from that unqualified scope. An oversized empty note also passes with correctly
selected paths because the owner parser has no byte bound. One real shared
managed-empty note remains a valid control.

These are controlled supported-input proofs, not a genuine Server bot candidate
or a production source claiming SDK 0.2.0. The ordinary clean Git default is
observed before edits; frozen released inputs and real shared prepared output
supply the proposed development transition. No future publication/version is
manufactured to create activity.

## Owner repair and regression

The source reader now supplies changed paths from the same captured immutable
parent used by its prior facts, explicitly selecting Git HEAD or Jujutsu @.
The wrapper loads the actual selected managed note before its existing scope,
full-catalog, range, peer, immutable-integrity and canonical checks. Missing
ancestry has no fallback. Git-only CI tests acquire no Jujutsu binary dependency;
a marker-only injected transport fixture exercises the separate JJ selection.

The owner note parser additionally requires well-formed text no larger than 8192
UTF-8 bytes. Existing empty-map, null, runtime, malformed, multiple and arbitrary
path refusals remain. The exact 8192-byte multibyte control is accepted, while
an over-bound UTF-8 body and ill-formed string are rejected. An independent guard
must inspect tracked note bytes, not infer safety from an empty working tree.

`validator-generated-scope.json.gz` retains the actual reviewed shared source `89e69c1a04c508c7ec8a22952742842d67951b5b` canonical output
and `writeChangeset(empty:true)` bytes for the already committed SDK fixture.
A real `git diff --no-index` over input-only before/after test trees supplies its
unified diff; only standard tree-label prefixes are normalized, never source or
note content. Those trees contain no VCS metadata and are not new checkouts.
The generated fixture binds the original SDK fixture SHA256 before consumption.

The focused regression command is:

```sh
node --test scripts/dependency-automation/source-intake-reader.test.mjs
```

## Source and delivery boundaries

A separate read-only real-Git replay at immutable main `648222c0` succeeds with the
actual prior `c0bcecb5` parent, ten selected tracked #101 paths and unchanged SDK 0.2.1
facts. It is explicitly a normal no-intake main observation, not a future SDK
candidate or hosted pass. The controlled forward patch tests own the changed
file/note behavior. Neither result asserts Rust, model, application or package
publication acceptance.

The existing production HIGH finding tracked by #99 remains separate and binding.
This slice adds no audit ignore, source exception, inherited coordinator approval
or dev-only/clean production claim. Fresh exact-head/main qualification and any
actual future bot generation remain required; no bot branch is manually patched.

## Verification

The original controlled boundary had four false acceptances before the repair.
The exact focused command now passes all ten cases, with zero false acceptances.
The complete dependency-intake suite passes all 35 cases. Owning `pnpm check`,
`pnpm test` (eight CLI/auth checks and 218 application unit tests), `pnpm build`
and `pnpm spec:check` pass. Formatter, workflow validation, doctor, QMD and
exact-main spec-first validation also qualify this source slice.

This changes Node intake plumbing and its owning specification only. No shared
application fixture, lifecycle, Storybook or runtime surface changes; product
Rust/model/browser and publication acceptance are not asserted by these checks.
Fresh hosted source CI remains required before normal merge and activation.
