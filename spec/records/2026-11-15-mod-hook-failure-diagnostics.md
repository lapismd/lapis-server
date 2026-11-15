# Mod hook failure diagnostics — 15 November 2026

PR #77's Linux ARM64 job `132746411643`, run `43692677089`, failed the
original `lifecycle_runs_hooks_after_files_reuses_receipts_and_best_effort_is_explicit`
case: **8 passed, 1 failed** in the registry target. Its first hook attempt
logged only `hook-failed`; the runner discarded whether process launch failed
or the child exited unsuccessfully. The exact unchanged case passed locally on
macOS ARM64. That is a non-reproduction, not Linux qualification.

The original workflow remained active until the other jobs completed. GitHub
then accepted a rerun of that exact failed job and its dependent jobs. The PR
head remains `4739af2c` so the original qualification is not replaced by a new
source revision. Its outcome remains pending at this receipt.

## Local diagnostic source boundary

The diagnostic patch distinguishes `spawn_failed` (error kind and numeric OS
code) from `exit_failed` (numeric exit code, or termination without one).
Neither category includes script contents, arguments or captured child output.
Existing operator-trusted hook path logging remains unchanged. Strict mode
still refuses startup; explicit best-effort still continues. Hooks run once.
No retry, ignore or assertion weakening is introduced.

The strengthened strict-hook target first failed **0 passed, 1 failed** because
the old error contained only `Mod hook failed`. After retaining the exit category,
the same exact target passes **1/1**. A missing-interpreter case additionally
proves launch-error reporting with OS code 2 and absence of script contents or
interpreter paths in the returned diagnostic.

- Exact target:
  `cargo +1.99.0 test --manifest-path crates/lapis-container-mods/Cargo.toml --locked --test registry hook_failure_blocks_strict_startup_and_does_not_reapply_installed_files`:
  **1 passed, 0 failed** after the change.
- Complete standalone crate: **18 passed, 0 failed, 0 ignored**, including
  **10/10** registry cases and the original lifecycle case.
- Strict standalone all-target Clippy with warnings denied: passed.

These local macOS results qualify the diagnostic source behavior only. They
do not identify or repair the original Linux ARM64 hook failure. Actual Linux
ARM64 execution and cold container acceptance remain separate evidence.
