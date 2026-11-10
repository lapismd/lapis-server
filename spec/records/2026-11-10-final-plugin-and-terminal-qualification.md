# Final local plugin and Terminal qualification

The approved Terminal API and upstream repairs are implemented. This record
separates local source/consumer qualification from package and image delivery.

## Source ownership and commits

- Notes `5b561ad1bbc5` bounds plugin-heavy startup and queues plugin installs.
- Notes `fe5ef33a` renews native AI sessions after successful disable.
- Terminal Host `e240ad5f8e891a3b30336aca49926992a08bde42` owns the private,
  Deno-free Rust PTY engine; Server uses that reachable exact Git revision.
- Terminal UI `9f1b9065` repairs focus, activation fencing and PTY retirement;
  `8a22c03d` transfers one pending/session owner across same-leaf View replacement.
- Server `297ef38a` implements the authenticated client/vault Terminal API;
  `d79309e5` bounds output-response lifetime to preserve HTTP/1 capacity.

The final Server UI consumer was qualified at working revision
`85fd7d3b4dcb76be5c21febd5e9c379f9fba94e6`, using seven integrity-verified
public-export archives. The supported validation hook keeps the registry
lockfile untouched. Archive provenance records the packed source snapshots;
subsequent upstream commits add qualification documentation and retain the
qualified production implementations.

## Defects and regression evidence

Startup reused neither the paged identity inventory nor the installed-plugin
content boundary, and ESM diagnostics read lazy chunks eagerly. The repaired
source avoids redundant point reads, excludes only `.lapis/plugins` from
content scans/watch reconciliation, and follows the eager ESM graph. Settings
remain loaded. The recorded post-migration cold open is 7.24 seconds and 373
fetches; the initial tombstoning of 733 previously indexed assets is one-time.
Previous tests checked eventual readiness rather than request counts against a
realistic installed chunk tree. New regressions assert those request boundaries.

Install requests now enter an App-owned FIFO with independent progress and
cancellation. Another plugin's queued install cannot cancel the finishing one.
Default native AI respects persisted choices, trust and safe mode. Re-enable
constructs a fresh feature session, leaving the old session fenced. Server
cleanup accepts only typed 501/UNSUPPORTED for an unselected native engine and
retains uncertain cleanup errors.

Widened browser acceptance initially exposed six Tasks failures. Finite,
cursor-resumed Terminal output releases HTTP/1 connections for editor admission;
AI cleanup/activation repair fixes startup Retry. All 15 Tasks cases now pass.
A later cold production case found a duplicate PTY during View replacement;
the new handoff regression fails before repair and passes after. The unchanged
browser assertion now observes one create and proves real keyboard input writes
the expected file inside the admitted vault. No assertion or timeout was weakened.

## Completed local validation

| Boundary                 | Result                                                                                                 |
| ------------------------ | ------------------------------------------------------------------------------------------------------ |
| Notes root               | 16 check tasks; 2,095 package cases; nine build tasks; specification/67 governance regressions pass    |
| Notes cold catalog       | 272 interaction/accessibility cases pass across four fresh shards                                      |
| App Profile              | 98 cases pass, including the before/after re-enable regression                                         |
| Terminal UI              | 27 cases, check, build, specification and prepack publint pass                                         |
| Terminal Host            | Five real PTY regressions and package/native required lanes pass                                       |
| Server native            | Four Terminal HTTP cases, all 132 Server package tests and all-target Clippy with warnings denied pass |
| Earlier native workspace | 774 passes, seven existing asset-gated ignores; unchanged Core/protocol behavior retains this receipt  |
| Server renderer          | Check/ownership, 110 unit cases, production build and specification pass                               |
| Server catalog           | 46 interaction cases, static build and four static-browser cases pass                                  |
| Complete hosted command  | 34 passes, zero failures, five existing opt-in skips                                                   |

The five hosted skips are the signed-plugin install asset gate, backup screen,
original-source screen, metadata screen, and actual ONNX/Linux provider gate.
They were already opt-in and are not counted as passing coverage. This change
adds no skip or quarantine.

## Running demo and remaining delivery gates

The healthy local container still runs the public image
`ghcr.io/lapismd/lapis-server:45f4c1b`, verified `linux/arm64`, digest
`sha256:ec459eec169b06b9b528a12bae7c271c61698039a2c556a3b58cef06d3e1564f`.
All Docker operations use `desktop-linux` at
`unix:///Users/stevejuma/.docker/run/docker.sock`.
An ignored, read-only Compose mount supplies the qualified local frontend.

The final live refresh opens the retained vault, shows the default AI ribbon,
and displays Terminal unavailable with the explicit server-update reason and
zero Terminal error panels. The public executable has no Terminal route. The
qualification tab was closed; user containers, volumes and vault files remain.

Package publication remains unapproved. The release cohort is API 0.2.5,
Workspace 0.3.3, App Profile 0.1.1 and initial Terminal UI 0.1.0. The Terminal
package is still absent from npm. Server's default-UI integration remains a
reviewable uncommitted diff until registry intake can resolve and lock those
released packages. Hosted CI, public image publication and acceptance of the
new ARM64 image remain delivery gates. No new Docker image was built locally.

Terminal scope governs authenticated API ownership and initial physical cwd;
shells inherit the server/container user's filesystem permissions.
