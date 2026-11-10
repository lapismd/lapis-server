# Content access under Core plugins

SRV-SRV-014 now explicitly places Content API and MCP under the public
`core-plugins` navigation group. The previous `plugins` ID made an unrelated
generic group. The host-owned definition changes only that ID; grant controls,
secret disposal, owner authority and the shared Notes renderer are unchanged.
Operator and CLI instructions use **Settings → Core plugins → Content API and
MCP**.

The cross-host backlog requires the same location in Desktop. Inspection of
Desktop found no external content-access settings plugin or listener yet; its
existing AI/Agents/Compute/Terminal settings already use Core plugins. This
slice does not invent a Desktop settings screen or qualify the pending Desktop
host integration, consent or listener lifecycle.

## Regression and acceptance

The existing production Content API and MCP stories now navigate through the
Core plugins group before exercising enablement, credential creation, secret
dismissal, revocation and disablement. The focused command was:

```sh
pnpm exec vitest run --project storybook stories/ServerHost.stories.ts \
  -t 'Content API and MCP'
```

Before the implementation edit: four failures across light/dark and desktop/
narrow, each unable to find the MCP section under Core plugins. After: four
passes. The existing real-host tests similarly locate the group and exercise
the HTTP/MCP controls, foreign-vault refusal, credential retirement and restart
with cold static, preview and development processes: three focused passes.

The initial type check rejected a Playwright-only `exact` option in the
Testing Library story query; an anchored accessible-name matcher corrected it.
The specification check initially required a server-chapter confirmation for
the CLI README path; that unchanged-contract confirmation is now included.

Final source checks include `pnpm check` (zero errors/warnings), all 105 UI unit
tests, the Vite production build and all 50 Storybook interaction/axe cases.
The complete hosted Notes lane (`pnpm exec playwright test -c
web/playwright.hosted-notes.config.ts`) passes all five cases. Formatting and
specification checks pass. These qualify this navigation-only surface; Rust
behavior and unrelated host lanes are unchanged. Visual baselines were not
changed.
