# MCP Apps verification

| ID          | Status      | Evidence                                                                                                                                                                                                                                                                                                                                                                    |
| ----------- | ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SRV-MCP-001 | Implemented | The MCP HTTP, CLI stdio and hosted content-access suites pass. They cover capability negotiation, text fallback, provider contribution, credential revocation and resource isolation from public static and PWA assets.                                                                                                                                                     |
| SRV-MCP-002 | Implemented | Four cold Chromium tests against hosted Rust and the official AppBridge cover Markdown refresh, a package without a Tasks contribution, read-only Tasks, guarded field edits, one-off completion and tracked recurring completion. The recurring test discards the write response and inspects the native receipt, verifying one daily-note entry on the user's local date. |

The external reference host uses two iframe origins and a form-capable sandbox.
The application bundles request no extra permissions and prevent outbound
network requests and form navigation. Individual external client products
require separate acceptance.

Both Storybook previews passed interaction and accessibility checks at desktop
and narrow widths. The static catalog test refreshes the sample note, changes
Inbox status, and completes a duration-tracked sample task through AppBridge.

The six static catalog tests pass with the packed Tasks plugin staged through
the Server build input. The MCP Apps catalog case verifies the white Notes
and Tasks bodies, icon-only refresh controls, the note breadcrumb and bottom
spacing, native task details at narrow and desktop widths, inline expansion,
checklist display and permission-scoped actions.

The Tasks layout assertions reproduced the duplicate detail border, 432 px
canvas, and missing top inset in the original bundle. The rebuilt package
passes with a single detail border, a 512 px canvas, and 8 px of vertical
list padding at the standard root font size.

The packed Tasks story checks that double-click renders the native inline notes
and metadata footer. It saves a schedule change from that footer, dismisses the
row on outside pointer and focus events, and opens the native sidebar through
the details button. Coverage includes the one-rem Markdown side inset and
Mira Properties styling. Hosted acceptance covers the Inbox control with write
and read-only grants. At 820 px the compact Tasks preview opens a full-width
detail pane and supports horizontal scrolling back to the list.

The packed preview reproduced 0.6 effective opacity on the title, deadline and
schedule badge when swipe-to-delete was unavailable. The Tasks plugin fix
restores full opacity to all three, and the task title matches the native
Lapis light theme's `#222222` color.
