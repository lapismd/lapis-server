# Terminal and plugin-heavy startup qualification

This work adds the approved authenticated Terminal control/output API and default
Terminal UI registration. Terminal Host owns the PTY engine at Git revision
`e240ad5f8e891a3b30336aca49926992a08bde42`; generic shell lifecycle is not
implemented in Server. Terminal UI source is qualified separately in the
`lapis-plugin-terminal-hosted` workspace at `01fcc14d`, followed by the view-owned creation fix `3ab39e5f`.

The initial public ARM64 image remains `ghcr.io/lapismd/lapis-server:45f4c1b` on
local Docker context `desktop-linux`. An ignored Compose override mounts local
static output for source-consumer qualification. This does not qualify a new
production image or provide a Terminal endpoint in the old executable.

Initial plugin-heavy vault startup measured 4,879 fetches over approximately
42 seconds, including 2,532 commands, 1,558 database requests and 724 plugin
file reads. A second capture isolated approximately 1,495 `getMeta` calls.
Packed identity-read repair reduced that count to 42, but repeated fallback
polling still traversed installed plugin assets and accumulated over 70,000
requests while the workspace remained open. The final Notes source also
excludes installed plugin assets from content discovery/watch reconciliation
and restricts ESM dependency diagnostics to the eager import graph.

Source regression evidence includes zero redundant unchanged-startup identity
point reads for 24 files (48 before), serialized overlapping installs with
independent queued cancellation, default native AI registration, no content
stats for plugin chunks, and no eager reads of lazy ESM chunks.

Terminal source checks cover client isolation, bounded input, relative cwd,
detach and principal retirement, dirty-close preservation, successful vault
close, direct output streaming, trust admission and cleanup. A separate real
PTY suite lives in Terminal Host.

Validation failures are retained rather than relabeled as passes:

- Initial workspace-wide native compilation failed in the unchanged CLI FIFO fixture
  (`rustix::fs::mkfifoat` unavailable on macOS), reproduced on parent `12f7b120`; POSIX FIFO creation retains the same assertions and its exact test now passes.
- One native queue capacity test timed out under concurrent heavy validation;
  its exact retry passes. Complete native workspace qualification passes serially: 774 tests, with seven existing asset-gated ignores.
- Concurrent Storybook preparation and consumer packing raced over API `dist`;
  both pass sequentially: 272 interaction/accessibility cases and six verified public archives. No assertion or timeout was weakened.

Remaining qualification and release gates are tracked in the canonical
verification table, including registry publication and ARM64 image acceptance.

The next cold Docker load after the one-time tombstone migration reaches the workspace in 7.24 seconds with 373 fetches (212 commands, 56 database calls, 96 file reads and 9 other calls). First repaired startup migrated 733 previous plugin-asset identities; that one-time path remains more expensive. The old public executable correctly has no Terminal route; native Terminal acceptance uses the separately built test host.

Native Clippy passes for all workspace targets with warnings denied. Server frontend check, 103 unit tests, 46 Storybook interactions, the production Storybook build and four static-browser cases pass. Full hosted acceptance is retained separately from source qualification.

Terminal sessions inherit server OS user permissions: vault scope controls API ownership and initial cwd admission, while OS/container policy controls filesystem access.
