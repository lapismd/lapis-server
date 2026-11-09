# Shared content MCP adapter

This private crate explicitly mounts `/mcp` for Server/Desktop integration. It
does not start a listener, choose vault roots, issue credentials or enable either
production host. [SRV-SRV-011](../../spec/src/current/server.md#srv-srv-011) owns the
contract; remaining consent, OAuth, host and write work lives in the
[content backlog](../../spec/src/backlog/content-automation.md).

`router(grants, public_origin)` serves controlled ephemeral read grants.
For combined hosts, construct `lapis_content_http::Boundary::new(grants, origin)`
and pass clones to each crate's `router_with_boundary`. MCP additionally takes a
`CancellationToken`. Clones share the native concurrency budget. Hosts cancel
the transport, shut down/drain grants on a blocking worker, and release clients
and listeners. Hosts still own TLS/proxies, connection limits and consent.

| Component      | Qualified source matrix                                 |
| -------------- | ------------------------------------------------------- |
| Server SDK     | Official Rust `rmcp = 3.9.0`                            |
| MCP protocol   | `2026-07-28`, stateless Streamable HTTP                 |
| Client         | Same official Rust SDK, `ClientLifecycleMode::Discover` |
| Authentication | Controlled native test grants, per-request bearer       |

Older initialized clients, public MCP client applications, OAuth protected-resource
discovery/authorization servers and CLI stdio bridging are not qualified yet.
The endpoint requires current MCP metadata and matching version/method/name
headers as specified by the SDK. It creates no sessions and rejects session IDs.
The shared boundary authenticates before body decoding and again at dispatch.

Tools are `capabilities`, `list_vaults`, `list_notes` and `read_note`. Inputs use
Protocol fields without the `operation` discriminator; list commands require
`page: { limit: 1..100, cursor?: string }`. Replies contain a Protocol
`ContentReply` in `structuredContent`; tool content failures set `isError` and
preserve the stable code/effect. Invalid tool names or argument shapes are MCP
parameter errors. Write commands are absent and refused.

`resources/templates/list` advertises `lapis://notes/{vault_id}/{path}`. Use an
admitted UUID and percent-encode the relative path; `resources/read` returns saved
Markdown with its strong version in `_meta["io.lapismd/contentVersion"]`.
`resources/list` is empty; find paths through the paginated `list_notes` tool.
The resource path is decoded once and validated by Core without URL normalization.

Requests are limited to 64 KiB, including unknown-length bodies, with 64 active
request handlers and a 15-second deadline. The shared native pool admits 64 jobs
without queueing and retains permits until cancelled blocking work finishes.
Core bounds note bytes and listing pages; JSON encoding adds wire overhead.
Replies are no-store, and MCP catalog/resource freshness is zero/private.

Run `cargo test -p lapis-content-mcp -p lapis-content-http --locked` for real
HTTP/client parity, protocol and boundary cases. Workspace tests and Clippy remain
required. No crate or npm package publication is introduced.
