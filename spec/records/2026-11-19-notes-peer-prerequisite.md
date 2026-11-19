# Notes peer prerequisite for dependency automation

The ordinary dependency reconciliation at Server source
`c0b43f5f711aff0328634b283167d314bf5a87e8` admitted API 0.6.10, UI 0.2.6,
Workspace 0.4.9 and Design Core 0.13.6. Renovate regenerated the lockfile but
the source-owned canonical guard correctly rejected the candidate: API 0.6.10
changes its UI peer declaration from `^0.2.1` to `^0.2.6`, and pnpm removes an
unused API 0.6.3 identity. Ordinary patch intake does not authorize either
first-party metadata evolution or unrelated identity cleanup.

This source prerequisite adopts only the peer-coherent API/UI pair. A one-use
receipt binds the immutable parent, unchanged policy, exact before/after
manifest and lock bytes, the two canonical chapters and the complete changed
path set. The catalog comparison normalizes only the reviewed API UI-peer floor
and stale API 0.6.3 identity; all other first-party packages, metadata, edges,
workspace settings and registry integrities retain their existing checks.

The private Server root gains no npm or product release intent from this
maintenance prerequisite. Workspace 0.4.9 and Design Core 0.13.6 remain
unchanged here so the shared coordinator still has a genuine compatible update
for ordinary bot-owned acceptance after this source change merges.
