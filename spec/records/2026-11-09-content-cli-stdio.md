# CLI MCP stdio bridge

Requirements: SRV-SRV-010/012. Parent shared MCP slice: `b1260eac`, PR #26.

`lapis mcp serve --stdio` relays the selected host's read tools and note resources.
The normal CLI dependency graph contains Protocol and the official SDK/network
client, without Core or either native host adapter. The source matrix is pinned
`rmcp 3.9.0`, HTTP discovery `2026-07-28`, and stdio discovery `2026-07-28` or
initialization `2025-11-25`. Tests assert the negotiated revision in both cases.

## Validation

Environment: Linux, Rust 1.99.0, Node 24.19.0 and system JJ 0.46.0.
CI retains its independent pinned JJ 0.45.1 and hosted/image gates.

- After correcting the fixture's SDK import path, the first official-client
  subprocess case failed against the existing CLI with an initialization
  connection closure. Both protocol paths pass after bridge implementation.
- A closed-stdout regression failed with stdin deliberately left open. Bounded
  runtime shutdown alone did not fix it: the SDK reports a failed send without
  closing the service. Cancelling the service on stdout failure, draining async
  transports, then avoiding an uninterruptible blocking-stdin wait fixes the case.
- The combined fixture initially expected an oversized line to close the SDK
  connection. Its documented behavior discards that line within the configured
  bound; the corrected case proves the following valid request still succeeds.
  The closed-stdout assertion still retains stdin and requires prompt exit.
- Six bridge process cases cover both official-client lifecycles, tool/resource
  catalog and saved-byte parity, scope/revocation without cached responses,
  protected protocol streams, startup/error privacy, redirect refusal, timeout
  without replay, line recovery and shutdown. All eight prior CLI read cases also
  pass after the runtime change.
- Every existing Cargo package version/source remains present. Native adapters
  stay dev dependencies; reqwest 0.13.5 serves the pinned SDK while ordinary CLI
  reads retain their existing reqwest 0.12.28 client.

`cargo test --workspace --locked` passed 760 tests, with 0 failures and 7
pre-existing model/helper ignores. Strict all-target workspace Clippy, standalone
CLI build, protocol drift/client typing, configure, specification/ownership,
Prettier and edited-crate Rustfmt all passed. No new ignore or assertion waiver
was introduced. Hosted/image CI remains a separate PR gate.

## Remaining boundaries

The stdio bridge uses Unix protected credential files; stdin carries protocol
messages and cannot also supply a credential. Non-Unix custody, named profiles,
public MCP client products, OAuth and distribution remain unqualified.
The next host slice must mount opt-in API/MCP inside the running hosted server.
Server and Desktop both require plugin settings for enable/disable, endpoint
status, admitted vault permissions and credential creation/expiry/revocation.
Those screens and native admission/drain are owning-host acceptance, not evidence
provided by this source subprocess fixture. Writes remain in the content backlog.
