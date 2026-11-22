# Native plugin operations

Compiled providers register one versioned operation catalog in App Core. HTTP,
MCP and CLI adapters discover and invoke that catalog. Tasks is the first
provider; installing a JavaScript plugin does not install a native provider.

## Add a provider

1. Own the adapter in the plugin repository. Implement Core's `OperationProvider`
   with `descriptors`, `available` and `invoke`, delegating to admitted native
   services through the supplied `Client`.
2. Declare a plugin ID and operation ID, integer version, permission such as
   `example:read`, effect, JSON input/output schemas and authored CLI help.
   Inputs are closed JSON objects. CLI argument metadata maps flags to fields;
   required fields must have mappings. Never take a native root or credential
   from operation input.
3. Register an `Arc` of the provider during host assembly, before publishing
   the immutable registry. Invalid or duplicate descriptors fail registration.
   The hosted assembly is in `crates/lapis-server/src/host/content/mod.rs`.
4. Keep admission in native services. `available` checks per-vault plugin
   enablement; `invoke` must preserve saved-version guards, editor fences and
   effect recovery. Handlers run on the shared blocking worker budget.
5. Test registration, permission refusal, disable/revoke and actual service
   execution. HTTP/MCP/CLI require no provider-specific dispatch code. Bundle a
   generated descriptor catalog only when that provider needs offline CLI help;
   otherwise commands and help come from live discovery.

The public Notes API supplies contribution declarations and an operations port.
Those declarations describe native capabilities; they do not turn renderer
callbacks or agent tools into external endpoints. Its package release and host
adoption are separate from the compiled Server integration.

## Connect to Tasks

In **Settings → Core plugins → Content API and MCP**, enable access, choose
admitted vaults and explicitly select `tasks:read` and/or `tasks:write` before
creating a credential. Note reads remain read-only. Task writes may also change
the configured daily note. A credential without plugin permissions gains no
plugin authority, and existing credentials never acquire added operations.

Tasks must be configured through the existing owner-controlled native Tasks
service. The server persists that per-vault configuration outside vault content
and restores it before listening after restart. An explicit Tasks disable
persists the disabled state. Credentials remain process-local: create a new
credential after a server restart.

HTTP discovery:

```text
GET /api/content/v1/operations?vault_id=<vault-uuid>
Authorization: Bearer <credential>
x-lapis-content-api: 1
```

Invoke a discovered operation with its advertised version:

```text
POST /api/content/v1/vaults/<vault-uuid>/plugins/tasks/operations/get
Authorization: Bearer <credential>
x-lapis-content-api: 1
Content-Type: application/json

{"protocol_version":1,"request_id":"<request-uuid>","version":1,"input":{"path":"Task.md"}}
```

Replies contain `protocol_version`, `request_id` and an `outcome` with status
`ok` or `error`. Bodies are bounded to 64 KiB; unknown inputs, mismatched versions,
foreign vaults, missing permissions and disabled providers are refused.

MCP exposes the same operations at `/mcp` as `tasks_get`, `tasks_complete`, etc.
Tool input adds the explicit `vault_id`. Writes also require a caller-chosen
`invocation_id` UUID. Keep that ID before sending the request so a lost response
can be investigated with `tasks_inspect` rather than replaying a mutation.

CLI examples:

```sh
lapis tasks complete --help
lapis help tasks
lapis help -k tasks
lapis --target https://notes.example --token-file token --json tasks get --vault <vault-uuid> --path Task.md
lapis --target https://notes.example --token-file token --json tasks complete --vault <vault-uuid> --invocation-id <request-uuid> --input-file complete.json
```

Host/credential flags precede the plugin command. Use a protected credential
file, not a token argument. Input files contain JSON objects, at most 64 KiB;
explicit field flags override file fields. Complete and skip use native journal
identities; inspect the retained receipt before requesting an explicit retry.
Commands never retry writes automatically.

The initial catalog has list/get/inspect, guarded checklist, planning, start,
deadline, inbox, repeat, tracking and state changes, plus complete/skip/retry.
Task creation and title/body edits, independently installable native execution,
Desktop listeners, OAuth and packaged CLI distribution remain later work in
`spec/src/backlog/content-automation.md`.

## Contribute an MCP App

Build the HTML as a self-contained view using the official MCP Apps SDK.
Register its entry operation and follow-up tools from the compiled provider's
operation descriptors in the hosted app registry. Use a versioned
`ui://lapis/<plugin>/<view>-v1` URI. Resource discovery follows the grant's
operation catalog. The view receives results and calls tools through the
external host, which owns iframe isolation and sends the usual MCP credentials
to Server.

The Server image includes the Notes view for `read_note` and the Tasks view
for `tasks_list`. For local source builds, run `pnpm build:mcp-apps` and set
`LAPIS_SERVER_MCP_APPS_ROOT` to the absolute path to `.local/mcp-apps-dist`.
Keep this directory outside the public static root and vault storage.

An external host requests `tasks:read` to show Tasks and also requests
`tasks:write` to enable edits and completion. The owner approves both scopes
for the selected vaults. After an uncertain completion, use **Check completion
status**. Retain the displayed request ID for native Tasks recovery.
