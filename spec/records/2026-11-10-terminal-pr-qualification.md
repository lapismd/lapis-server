# Native Terminal PR qualification on current main

The native API and root Compose demo are integrated on Server main
`121fa45b47f6d3ca3fa002540adcd4bce3ec010c`. The merge preserves the newer
hosted content-access contracts, generated protocol fields, verification rows
and Pages deployment recovery. Generated bindings were rebuilt from both the
content and Terminal source contracts.

Fresh locked registry installation, eight demo checks, protocol generation/check,
canonical specification, formatter, renderer check, 105 unit cases and production
build pass. All 142 Server package Rust tests, three Core Terminal cases, five
CLI security cases and strict all-target Server Clippy pass. The four Terminal
HTTP cases include bounded connection lifetime and authentication revocation.

The PTY engine source is pinned to the exact reachable commit included in
Terminal Host PR #2, merged as `894ceb2eb214a17e4b6b949a1bfa601124f90da6`.
The separately qualified Terminal UI remains outside this registry intake while
its package is unpublished. The local qualification frontend and its source
branches remain preserved. This PR does not add an unpublished npm dependency.

Main's image workflow publishes a new public image after merge. That automatic
publication is distinct from npm publication and the running local demo.
