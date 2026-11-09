# Initial content CLI reads

Requirements: SRV-SRV-004/010. Parent HTTP slice: `b9ac7ae6`, PR #23.

The private `lapis-cli` crate builds `lapis` as a Protocol/HTTP client, with Core
and the shared HTTP adapter used only by its subprocess tests. It has explicit
targets, protected file/stdin credentials, bounded reads, JSON and byte-preserving
human output, strict response identity/type checks and stable errors/exit codes.

## Validation

Environment: Linux, Rust 1.99.0, Node 24.19.0 and system JJ 0.46.0.
CI retains its independent pinned JJ 0.45.1 and hosted/image gates.

- Before implementation, `cargo test -p lapis-cli --test reads --offline`
  compiled the placeholder binary and failed all three initial cases. The same
  target passed all three after implementation and compile/format corrections.
- The expanded target covers eight subprocess cases against real HTTP: discovery,
  pagination, exact bytes/versions, stable error streams, credential privacy and
  Unix permissions/type checks, malformed remote replies, redirect refusal,
  bounded chunked bodies and timeouts without retries.
- One adversarial-response case initially failed because the stub combined
  manual transfer framing with a fixed body. A real unknown-length body stream
  replaced that invalid fixture; the exact case passed without changing the
  CLI's expected error codes or assertions.
- A large JSON note sent to a closed stdout pipe failed the new pipe regression
  1/1. Preserving the serializer's underlying IO error kind restores successful
  pipe termination; the exact case then passes in JSON and human modes.
- Adding Clap 4.6.7 initially moved the existing bitvec lock entry to 1.0.1.
  It was restored to 1.1.1 before final checks; existing dependency versions and
  pinned Git sources remain unchanged.

`cargo test --workspace --locked` passed 743 tests, with 0 failures and 7
pre-existing model/helper ignores. The final private stderr-failure handling
adjustment was followed by all eight CLI process cases, an independent CLI build
and strict workspace Clippy; all passed. Protocol drift/type checks, configure,
specification, Prettier and edited-file Rustfmt passed. The normal CLI dependency
tree contains Protocol and no Core, HTTP-host or native vault engine. No new
ignore or assertion waiver was introduced.

## Remaining boundaries

The CLI has source-level subprocess acceptance against shared native test
grants. Production Server/Desktop mounting, owner consent/login, persistent
credentials, named profiles, Desktop discovery, writes, MCP stdio, completions
and binary/image distribution remain unqualified in the content backlog.
