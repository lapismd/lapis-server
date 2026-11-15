# Server repository remote startup and HTTP admission

SRV-SRV-004/015/016 now compose the shared native remote services in the server
executable's profile startup. This slice qualifies native startup and admitted HTTP custody/SSH separately
from browser UI, Desktop and overall application acceptance.

## Baseline and focused evidence

`node .local/remotes-validation.mjs test --locked -p lapis-server --test
http_files repository_remotes` initially reports zero passes and one failure:
GET remote inventory returns 404. After adding the bounded routes and native
adapter, the exact target passes once, with zero failures.

Two real HTTP/native startup scenarios then pass, with zero failures. They use
actual AES-256-GCM custody and reconstruct the host repeatedly with its correct,
absent and incorrect key. Selection, stable installation/owner identity and
opaque credential references survive restart. Save, replace and forget do not
return secret bytes. Saving retains manual mode, no observation/publication,
and another vault's connection stays unconfigured. Recursive installation-file
inspection finds none of the submitted tokens. Separate tests exercise protocol,
origin, foreign principal/vault/generation, rejected path fields, bounded input,
working-copy backup status, no-store responses and detached-client refusal.

Adding a standalone startup test binary with the same stem as an HTTP submodule
causes one Rust module-resolution diagnostic (E0761). The submodule is renamed
`remote_routes`; the acceptance function and assertions are preserved. The next
HTTP run passes both the original regression and owner-cookie/CSRF case. Two
private-startup scenarios additionally pass for unsafe key locations/symlinks,
public directory permissions and missing identity over retained ciphertext.

## Composition and remaining gates

Private storage uses physical 0700 directories and a stable installation/owner
identity. It never silently remints an identity over existing ciphertext or
selection. One installation owner backs authenticated cookie sessions and the
legacy installation bearer token. Attachment authority remains principal/vault/
generation scoped. Requests cannot select an owner or native path.

Operator key files must be outside profile/vault storage. Missing/wrong keys lock
custody; an unsafe location refuses startup. SSH trust stays outside vaults and
host-key verification is preserved. `openssh-client` is added to the runtime
image. Operator provisioning/persistence/recovery are documented separately.

## Final qualification

The complete Rust workspace passes 868 tests with zero failures and 10 explicit
opt-in ignores across 174 suites. Strict all-target whole-workspace Clippy with
`-D warnings` and build pass. The full run precedes a final refinement of the
opt-in HTTP transport test; Clippy/build and actual SSH include that refinement.
The existing CLI executable-fixture race interrupts the first full run at
658 passed/1 failed/9 ignored across 132 suites. Its isolated repair and complete
passing rerun are retained in the separate CLI fixture receipt and commit.

All three real SSH scenarios pass: existing Core original, existing Core backup,
and a new real HTTP/SSH case covering both targets through production Host/router.
The HTTP scenario refuses wrong host keys, proves save/test/fetch/review never
publish, publishes exactly the reviewed branch/commit and excludes unsnapshotted
original files and other bookmarks. Original checkout/tree-state bytes stay
unchanged. A held real fetch outlives request-waiter cancellation and client
detach; a replacement attachment observes its completed fetch. Both target push
responses are deliberately lost, then cold Host reconstruction recovers the same
native publication ID by observation without another receive. Reusing a consumed
review refuses. Disposable server, agent and keys are retired.

Web type/App-ownership checks pass. A reused node_modules symlink initially
causes Vite's filesystem admission to refuse CSS imports in two suites (124 other
cases pass). It is preserved as an ignored link, and an independent frozen
registry install is created in this workspace. The exact two suites then pass
8 tests; the entire web suite passes 132 tests in 31 files. Web build, eight demo
tests, protocol/configure/spec/format and changed-source Rustfmt pass. QMD search
and doctor pass (10 checks, existing alias warning), using spec-validator 0.2.0
and Node 24.10.0. No filesystem admission rule or assertion is relaxed.

Rust commands use immutable tracked archives, Rust 1.99.0 and the existing
isolated cache `604a978da80314ea5eba` in remote Docker image
`sha256:6031c86d7c26d9e7011c958374462c600b10aa6a586ad7196d741756df0a5a32`.
Final strict/build/SSH source is snapshot `129a44ad7a6ccb2b673128eb728da2dfa23e5016`.

## Remaining gates

Operator/image tooling and hosted CI remain required before landing. These HTTP
and cold Host cases do not qualify a cold executable/container/browser deployment,
live HTTPS/forge login or Desktop. Automatic backup synchronization, actual browser adoption,
Desktop OS custody/routes and both cold application gates remain unfinished.
