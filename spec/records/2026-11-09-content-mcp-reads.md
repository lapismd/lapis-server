# Shared content MCP reads

Requirements: SRV-SRV-004/009/011. Parent CLI slice: `19727d3f`, PR #24.

The private `lapis-content-mcp` crate mounts read tools and note resources over
the official `rmcp 3.9.0` SDK. It qualifies stateless protocol `2026-07-28` with
the same SDK's discovery client. HTTP and MCP can share one `Boundary` for
credential/origin checks and native capacity. No production host is mounted.

Primary references checked on 2026-11-09:

- [Official Rust SDK](https://github.com/modelcontextprotocol/rust-sdk), with the
  exact published 3.9.0 source inspected locally for transport and handler APIs.
- [Streamable HTTP](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http),
  including per-request metadata and matching method/name/version headers.

## Validation

Environment: Linux, Rust 1.99.0, Node 24.19.0 and system JJ 0.46.0.
CI retains its independent pinned JJ 0.45.1 and hosted/image gates.

- Two initial real-HTTP cases failed against an empty placeholder router with
  404 instead of authentication/read responses. The same cases pass after the
  implementation and fixture corrections.
- Initial raw fixtures omitted mandatory method/resource-name headers. The SDK
  rejected them with 400; fixtures now send the specified headers. The official
  client fixture initially included an extra Bearer prefix; using the SDK's
  documented token argument fixed its 401. Neither required weakening admission.
- JSON-RPC batch rejection uses the pinned SDK's 415 response, not the initial
  fixture's assumed 400. Batches remain refused and never execute content calls.
- Eight real-HTTP integration cases cover official-client discovery and reads,
  catalog/schema parity, exact saved bytes and versions across HTTP/tools/resources,
  scope/cursor isolation, revocation/expiry, write and argument refusal, resource
  traversal/encoding, body/header limits and strict protocol metadata.
- Three transport cases cover revocation after preflight while the body is
  pending, all 64 request slots occupied with cancellation/recovery, and the
  15-second slow-body deadline. The ten existing HTTP policy/admission/read cases
  still pass after extracting the shared boundary; cancellation retains native
  permits until the blocking work completes.
- Cargo retains every prior package version/source; only SDK dependencies and
  required feature edges are added. No new ignore or assertion waiver is used.

`cargo test --workspace --locked` passed 754 tests, with 0 failures and 7
pre-existing model/helper ignores. Strict all-target workspace Clippy passed.
Protocol drift and client typing, configure, specification/ownership checks,
Prettier and edited-crate Rustfmt passed. The SDK/client, HTTP and CLI acceptance
all ran together in that workspace result. Hosted/image CI remains a separate
PR gate; these local checks do not qualify either production host.

## Remaining boundaries

This is source-level acceptance using ephemeral native test grants. Production
host mounting/consent, OAuth protected-resource and authorization-server discovery,
persistent credential custody, public client products, legacy MCP compatibility,
CLI stdio bridging, writes and Desktop intake remain in the content backlog.
The read adapter does not qualify general remote MCP authorization or deployment.
