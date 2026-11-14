# Bundled CLI and host setup qualification

The installer uses a private per-user copy, owned symlink/receipt and exclusive
mutation lock. It refuses foreign or modified files, repairs missing owned
entries, and changes Bash/Zsh PATH only on request. The server container includes
the command on PATH; browser settings install on the browser user’s computer.
Release automation prepares source-bound archives for the three supported
platforms and refuses differing existing release bytes.

## Exact source and consumer checks

- `cargo test --locked -p lapis-cli --test installation`: 8 passed; the complete
  CLI suite passes 24 cases. Subprocesses cover setup/removal, repair, foreign
  entries, shell symlinks, explicit source switching, tampered receipts/copies,
  JSON path escaping and exact restoration without a final newline.
- `cargo test --workspace --locked`: 809 passed, 0 failed, 7 existing ignored.
  `cargo clippy --workspace --all-targets --locked -- -D warnings` passes.
- `node --test scripts/cli-distribution.test.mjs`: 4 passed, including standard
  archive contents, source/checksum refusal and reproducible retry bytes.
- Real content-access plugin target: 1 passed. `pnpm check`: no type errors or
  warnings, ownership audit passes. `pnpm test`: 132 Vitest and 4 archive tests
  pass. `pnpm protocol:check` and `pnpm test:plugin-operations` pass.
- `pnpm build`, `pnpm build-storybook` and static catalog acceptance pass
  (4 static cases). Full `pnpm test:storybook` passes 58 interaction/axe cases.
  One initial narrow-family dynamic import failure cleared at its exact
  28-case target and a fresh full run; the failed attempt remains in local logs.
- `pnpm test:hosted` qualified each family with 39 passed and 5 existing skips.
  The final-cohort attempt had two Tasks failures (a configuration fetch during
  teardown and missing startup content). Both exact targets pass unchanged;
  the entire Tasks family then passes all 15 and the real cold static-server
  case passes. No assertion, timeout or test infrastructure was weakened.
- `pnpm spec:check` and Rust formatting pass; validator doctor/QMD search ran
  against the latest Deno-free registry validator 0.2.0.

## Actual image and archive acceptance

The final isolated image build uses source snapshot
`9a34ed8c82890f6ec0ced4aa1fbc4c38ed34fc36` and the explicitly projected UI
public-build candidate, SHA-256
`87b70824eba9a0ab2ba48fea74739bb758de514609ad11ae1246940d07d33159`.
Tracked dependency manifests/locks retain ordinary released npm ranges. The
shared UI contains no App API dependency, preserving server’s existing cohort.

`bash scripts/test-image-slim.sh lapis-server:cli-validation` passes CLI
help/version under UID 1993, read-only installation, original service startup,
Mod injection/hooks, shutdown and size checks. Image size is 404,303,872 bytes,
below the existing 500 MiB gate. A real extracted Linux x64 CLI archive installs,
executes through PATH and uninstalls in a temporary HOME without credentials.

## Release boundary

UI `/cli` needs its governed package release and consumer range adoption before
ordinary registry-only builds. Multi-platform workflow execution and official
archive publication remain release gates; macOS is build-only here. The desktop
consumes retrievable CLI source commit
`dda488690a3db9567ed90088152baf2e0dbc0daf`. No npm publication, release asset
upload, deployment, installed-app replacement or visual baseline update ran.
