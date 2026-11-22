# MCP Apps

## SRV-MCP-001 — Scoped application resources

**Requirement.** The hosted MCP adapter MUST register packaged, versioned
`ui://` resources for built-in tools and admitted native plugin operations.
Discovery and reads MUST require the existing MCP credential and recheck
provider admission. Tool results MUST retain their text and structured content.
The adapter MUST decline duplicate or unknown bindings.

Current per-request clients opt in through the MCP Apps MIME capability. Legacy
stateless HTTP clients can discover optional UI metadata. The stdio relay can
carry the metadata and resource contents. The single-file Svelte builds live
outside public static and PWA roots. They request no additional iframe
permissions or network origins. The external MCP host sandboxes the resources
and mediates tool calls.

### Acceptance details

- MCP Apps-capable clients discover the packaged resources for the tools permitted by their grant.
- Missing or revoked credentials cannot read bundles.
- Ordinary MCP clients retain the same tool results.

## SRV-MCP-002 — Inline notes and Tasks

**Requirement.** The Notes view MUST render saved Markdown in Mira safe mode
and refresh through `read_note`. The Tasks view MUST use scoped existing
operations to read tasks, edit planned date, deadline and Inbox, and complete
open tasks with the existing Tasks controls and tracking prompt.

Tasks edits require a saved version and an invocation UUID. Completion uses
the user's local calendar date and the native journal, including recurrence
and daily-note behavior. The view reports success after a completed receipt.
It uses `tasks_inspect` after an uncertain completion and never resends a write.
Storybook MUST render both packaged views in a sandboxed external conversation
fixture with AppBridge and sample data.

### Acceptance details

- The Notes view renders safe Markdown and refreshes the same saved note through MCP.
- Existing Tasks permissions and saved-version checks guard all writes.
- Recurring and tracked completion use the native journal.
- Inspection recovers a lost response without repeating the write.
