# Hosted background jobs source acceptance

This receipt extends the [native runtime qualification](2026-11-24-plugin-background-runtime.md)
for the Server consumer. Source cohort `02d271eb199c745801fa7cf26aa749b167ea3b4d`
ran in Omarchy Docker with explicit verified Notes development archives.
Committed manifests and locks retain registry ranges; package publication and
registry-only consumer refresh remain separate gates.

## Source and real host evidence

- Three focused transport cases pass: authenticated native admission, passive
  detach and no retry after an uncertain manual request. Renderer metadata mode
  exposes no background-job port.
- The real installed-plugin acceptance passes alone and in the wider hosted
  suite. Shared production settings require explicit approval. A five-second
  native job publishes a guarded property after browser sign-out; a cold Server
  restart restores its private checkpoint and the next run publishes count two.
- The first acceptance fixture exhausted the broker-call quota by polling it in
  a busy loop. The corrected CPU delay relies on engine interruption. The next
  attempt exposed a literal assertion against valid quoted YAML; parsed numeric
  values and the retained Markdown body now prove the intended contract.
- Type/Svelte checks, all 244 browser unit cases, specification, generated
  protocol/OpenAPI and production build pass. The build requires a bounded
  6 GiB Node heap inside the 12 GiB worker; the original heap exhaustion remains
  recorded separately from functional failures.
- All 110 cold catalog interaction/accessibility cases pass; static Storybook
  builds and all ten static-browser checks pass.
- Full `test:hosted` and `test:plugin-operations` pass, including original
  authenticated UI, installed-plugin verification, file changes, Tasks,
  Properties, native History, static/PWA and MCP Apps cohorts. Existing explicit
  quarantines and opt-in cases remain unchanged.

## Exact PR dependency cohort

Native runtime PR #152 uses frozen registry dependencies independently of the
new optional browser adapter. Its first hosted CI run timed out waiting for the
Sign in heading after an injected 403. The exact test passes three cold repeated
runs with one worker against source `15aae4273b66da411a6b2704d41fdd5d984c45d5`
and its frozen registry lock. No timeout, assertion or production policy changed;
the original hosted failure is retained and a green hosted rerun is required.

## Remaining gates

Adopt released Notes API, Workspace and App Profile versions, then complete
registry-only frozen consumer acceptance and hosted PR checks. Desktop native
metadata default and window/Quit qualification remain Desktop-owned. This receipt
does not claim an npm publication, a signed plugin release or live deployment.
Visual approval remains pending; no baseline changed.

## Frozen registry follow-through

The public Notes API 0.6.15, Workspace 0.4.11 and App Profile 0.3.1 cohort is
published and registry adoption PR #155 is merged. The composed hosted-controls
and terminal-completion source is `c5a57e3a0c6eee1995d459d2aa0fd7c93c834643`,
pack SHA256 `bb1af7431fbe4cdda4310c880c90d77182e82c579c6c7ee4e02d6c65ddeedb36`.
Container `lapis-plugin-jobs-server-controls-20261125e` uses frozen registry
installation in Docker context `omarchy`, with no Notes development overlay.

Specification/doctor, formatting, dependency-intake tests, all 244 unit cases,
110 cold catalog interaction/accessibility cases, static build and ten static
browser checks pass. The exact native installed-job browser test passes alone
and in full hosted acceptance: 54 enabled cases pass and the same six existing
opt-in/quarantined cases remain excluded. Plugin-operation checks pass. Execution
tests pass 10 cases and Core job acceptance passes 10 cases. The full Rust
workspace and strict Clippy pass.

Earlier composed attempts stopped on canonical chapter size/mapping validation.
Their logs are retained. The two unpushed source slices were combined so the
local immediate-parent specification range matches the intended feature PR.
No mapping exemption or limit change was made. Local log collection briefly
exhausted disk; the remote worker continued and its full log was recollected.
Released signed Tasks and Desktop native/default acceptance remain separate gates.
