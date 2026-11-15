# Native credential provider composition

Core composes host-owned raw-token custody and an injected jj-forge account
manager. Shared inventory, endpoint/account scope, identity-preserving
replacement and SDK device-flow policy remain transport-independent. Server
injects its installation/owner-bound encrypted store; no native keyring or
plaintext fallback is constructed when its key is missing or incorrect.

The checked SDK source is jj-store PR #87 at
`6649b7e95ee58823ba28f0a9ca01e507b458edd9`. All 18 locked SDK coordinates and
native manifests/configuration select that immutable source. The initial source
qualification retained its pending merge gate. Hosted CI `43712203583` passed
at that exact head. PR #87 then merged as
`9a804c393487cf8ae6c5535ae8c4bec54b38d5e5`; both commit trees are
`4923d402d4c16d678d6ef30162dab185a747aa7b`. The source pin is now merged,
without changing qualified source bytes or rebuilding under another coordinate.

## Narrow evidence

- Baseline `node .local/remotes-validation.mjs test --locked -p lapis-app-core
--test repository_remote_provider` fails once: the native composition and host
  token-store trait do not exist.
- Initial implementation compiles against the previous SDK until the expected
  missing `replace_token` method; intake selects the qualified SDK candidate.
- First complete fixture compile has 34 diagnostics from importing Protocol's
  root instead of its public `repository_remotes` module. Corrected the import.
- Exact rerun: five Core scenarios pass, zero failures/ignores. They cover raw
  lifecycle, restart and cross-method/endpoint refusal before resolution;
  GitHub/Enterprise/Forgejo identity replacement and both host identities;
  locked custody retaining SSH; real SDK device polling intervals, completion,
  expiry and cancellation; public inventory excludes token/device/session bytes.
- `node .local/remotes-validation.mjs test --locked -p lapis-server --lib
credentials::remotes_tests`: two scenarios pass, zero failures. They use actual
  encrypted records and owner-bound injected forge custody. Raw and forge
  save/replace/forget survive cold store reopening; foreign owners refuse;
  absent/wrong keys remain locked and reopening with the correct key recovers.
  Recursive inspection finds no submitted token bytes in installation files.

Provider transports are fixtures. These tests do not qualify real forge login
or authenticated Git transport. Cold encrypted-store reopening is distinct
from cold server/browser/Desktop acceptance.

## Wider checks

The slice changes shared native credential policy and its Server adapter, so
qualification escalates to the entire Rust workspace plus strict all-target
Clippy/build and both real original/backup SSH regressions. Commands use immutable
tracked archives, remote Docker image
`sha256:6031c86d7c26d9e7011c958374462c600b10aa6a586ad7196d741756df0a5a32`,
2 CPUs/8 GiB and the existing isolated cache `604a978da80314ea5eba`.

- `node .local/remotes-validation.mjs test --locked --workspace`: 860 passed,
  zero failed, nine existing opt-in ignores across 173 suites.
- A final expiry-projection refinement makes inventory show an expired device
  code before another poll. The five focused provider cases pass again; the
  860-test run precedes that small projection refinement.
- `node .local/remotes-validation.mjs clippy --locked --workspace --all-targets
-- -D warnings`: passes on the final source.
- `node .local/remotes-validation.mjs build --locked --workspace`: passes.
- `node .local/remotes-validation.mjs --ssh`: both real original and backup
  cases pass. Disposable agent, host and keys are retired.
- `pnpm protocol:check`, `pnpm spec:check`, `pnpm format:check`, native manifest
  configure and changed-file Rustfmt checks pass. Initial specification checks
  found two not-yet-created receipt links, then one incorrect relative link;
  both are corrected and the final specification lanes pass.
- Node 24.10.0; Rust 1.99.0; spec-validator 0.2.0. QMD search succeeds;
  doctor passes ten checks with the existing alias warning.

## Scope and next gates

SRV-SRV-004 accepted work and SRV-SRV-015/016 custody/lifecycle contracts are
retained. The two credential requirements remain Partial. Private account
metadata is separate from encrypted records. Core reserves no provider state
in vaults, browser storage, exports or checkpoints. The existing accepted-work
fence and pending-publication holds guard all provider effects.

No HTTP/Tauri route is enabled by this slice. Complete admitted owner/startup
composition, the Desktop OS adapter, native automatic-backup scheduling,
production UI adoption and cold real transport/credential acceptance in both
hosts before the overall release. Visual baselines remain unchanged.
