# Content specification chapter

Date: 2026-11-09. Jujutsu change `kxkrxxpy`, based on main `15d4040c`.

SRV-SRV-005 through SRV-SRV-012 moved unchanged from the nearly full hosted-server
chapter to `current/content.md`. The old anchors remain as links; immutable
historical receipts were not rewritten. SUMMARY, the Storybook mirror/order and
the canonical chapter matcher include the new topic. No requirement, runtime
behavior, release policy or verification status changed.

The extracted documentation change passes `pnpm spec:check` and
`pnpm format:check`. The same navigation configuration also passed the combined
stack's 46 Storybook interaction/axe cases, catalog build and four cold built
static cases. Visual baselines were unchanged. The native host controls and
their qualification belong to the following stack layer.
