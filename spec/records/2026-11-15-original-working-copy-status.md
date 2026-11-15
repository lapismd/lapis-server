# Original working-copy status without capture

Change `pwytqwvo` implements the admitted original working-copy observation under
SRV-SRV-004/016. The existing jj-store working-tree Observer owns physical scans,
ignore/sparse handling and native parent comparisons. Core adds no scanner or
diff implementation. Its source witness holds a weak vault reference, no
permanent work lease, and no original mutation capability. The accepted worker
survives response cancellation and fences delivery to retired clients.

The source-only method supports existing Git-backed JJ originals. It returns
unavailable for a vault without an original and never initializes a repository.
The JSON status identifies `source: original`; eventual UI integration must use
the matching original reader when opening its operation. The managed external
backup does not yet support this observer's workspace profile; that remains an
upstream capability/adoption gap, and no false zero totals are supplied.

Before implementation, the exact target
`LAPIS_REMOTES_VALIDATION_CACHE=604a978da80314ea5eba node
.local/remotes-validation.mjs test --locked -p lapis-app-core --test
repository_working_copy` reported four E0599 missing-method errors. The final
strengthened target passes 2 cases with zero failures. It recursively compares
all operation-head paths/bytes, checkout and tree-state bytes, and the saved
unsnapshotted content. The real fixture reports 2 files and +2/-1 without
capturing. Detached calls refuse and a new attached client succeeds. No-original
reads preserve the vault and release accepted work.

Validation uses immutable revision-labelled Linux archives, Rust 1.99.0 and the
pinned tool image
`sha256:6031c86d7c26d9e7011c958374462c600b10aa6a586ad7196d741756df0a5a32`:

- Full Core/Protocol/Server tests: 797 passed, zero failed, seven existing opt-in
  ignores across 155 suites.
- All-target strict Clippy for those three crates: pass.
- Native Server build: pass.
- Canonical `pnpm spec:check`, App ownership and changed Markdown formatting:
  pass. Doctor reports 10 passes and the existing script-alias warning; QMD
  returns the owning server chapter.
- Changed Rust files pass direct Rustfmt. An extra whole-workspace Rustfmt check
  reports existing formatting drift in 35 unchanged files. Exporting parent
  `3296b49973c3602ecae5c0b0b0a451cbfe83ef80` reproduces the same 35-file set,
  with no added or resolved failures. The required CI formatter is Prettier;
  no unrelated Rust source was reformatted.

These checks qualify the native source boundary only. Production UI adoption,
HTTP/Tauri adapters, remote orchestration and both cold application acceptance
remain open. Hosted Server CI independently found a missing D-Bus development
library through jj-forge's unconditional OS backend; the owning optional-custody
source fix is being qualified separately before the next consumer pin.
