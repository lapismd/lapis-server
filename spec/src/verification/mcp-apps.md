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
