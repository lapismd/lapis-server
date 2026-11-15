# Merged native remote engine and encrypted forge adapter intake

Change `potnsqlq` consumes jj-store merge
`7ac5b142fdbb4378dfc48b60cfbb7324bea1c5a0` through portable Cargo Git pins.
Native source PR #80 merged after every executable GitHub check passed at
`7d8dee98446a1289bb4c38ee5584161b1b52db0f`; `jj diff --summary` between that
checked head and the squash merge is empty. The owning guide, configure gate,
manifests, lockfile and SRV-SRV-002 now select the merged revision. No crate tag,
external path or public crate publication is introduced.

The new Server adapter injects encrypted custody through jj-forge's existing
SecretStore trait. Its owner is selected by native composition, never a DTO.
Missing keys refuse adapter construction; there is no native-keyring or
plaintext fallback. Full account identity remains authenticated by the custody
source qualified in the [forge receipt](2026-11-15-encrypted-forge-custody.md).
Actual manager construction, authenticated routes and sign-in are still separate.

The exact adapter baseline
`node .local/remotes-validation.mjs test --locked -p lapis-server --lib
credentials::forge_adapter_tests` reports four E0599 missing-method errors from
its immutable pre-implementation archive. The corrected exact target passes two
cases: injected owner isolation/forgetting and locked-store refusal. The Core
selection target additionally proves agent/default SSH selection needs no forge
account and remains manual; incompatible HTTPS-token/SSH pairing is refused.

An initial concurrent cold-cache Core run failed fetching the pushed Git
revision (`revision ... not found`), while the independent archive completed
that fetch and reached the expected adapter compilation errors. A later serial
retry passed; no transport or source checks were weakened. Before merge-pin
intake, all 792 Core/Protocol/Server cases passed against the checked PR tree.

Final validation repeats against the actual merged Git revision, using the
isolated, revision-labelled Linux snapshots, Rust 1.99.0, Node 24.10.0 and pnpm
10.34.5. A task-owned existing compiler cache is reused across Git-pin changes;
Cargo fingerprints and locks remain authoritative, and every run receives its
own immutable source archive.

- `node .local/remotes-validation.mjs test --locked -p lapis-app-core --test
repository_remotes`: 7 passed, zero failed.
- Exact adapter command: 2 passed, zero failed.
- `node .local/remotes-validation.mjs test --locked -p lapis-app-core
-p lapis-app-protocol -p lapis-server`: 792 passed, zero failed, seven existing
  opt-in ignores across 153 suites.
- Strict all-target Clippy for all three crates: pass.
- `node .local/remotes-validation.mjs build --locked -p lapis-server`: pass.

This qualifies native source intake and the injected custody adapter. Shared
remote command orchestration, manager startup/provisioning, both host adapters,
production UI adoption, scheduler migration and cold application/real transport
acceptance remain outstanding. SRV-SRV-015/016 remain Partial.
