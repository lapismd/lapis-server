use lapis_app_core::services::plugin_operations::OperationProvider;
use lapis_app_protocol::{
    ServiceError,
    content::ContentReply,
    http_docs::{Auth, Body, Endpoint, schema},
};
use schemars::{SchemaGenerator, generate::SchemaSettings};
use serde_json::{Map, Value, json};

pub fn openapi_document() -> Value {
    let mut generator = SchemaSettings::draft2020_12()
        .with(|settings| {
            settings.definitions_path = "#/components/schemas".into();
        })
        .into_generator();
    let mut paths: Map<String, Value> = Map::new();
    let mut entries: Vec<_> = lapis_app_protocol::http_docs::register::iter::<Endpoint>
        .into_iter()
        .collect();
    entries.sort_unstable_by_key(|entry| (entry.path, entry.method));
    for entry in entries {
        let path = entry.path.replace("{*", "{");
        let operation = operation(entry, &path, &mut generator);
        let item = paths
            .entry(path)
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .expect("path item");
        assert!(
            item.insert(entry.method.into(), operation).is_none(),
            "duplicate documented route: {} {}",
            entry.method,
            entry.path
        );
    }
    let operations = lapis_task_operations::TaskOperations
        .descriptors()
        .into_iter()
        .chain(lapis_app_core::skills::provider::SkillsProvider::default().descriptors())
        .chain(lapis_app_core::services::content::NoteWrites.descriptors())
        .collect::<Vec<_>>();
    let mut scopes = Map::new();
    scopes.insert("mcp:read".into(), json!("Read explicitly approved vaults"));
    scopes.insert("offline_access".into(), json!("Refresh this connection"));
    for permission in operations
        .iter()
        .map(|operation| operation.permission.as_str())
    {
        let description = match permission {
            "notes:read" => "Search notes and read frontmatter",
            "notes:write" => "Create, replace, and edit frontmatter in Markdown notes",
            _ => permission,
        };
        scopes.insert(permission.into(), json!(description));
    }
    json!({
        "openapi":"3.1.0",
        "info":{"title":"Lapis Server HTTP API","version":lapis_app_protocol::PROTOCOL_MAJOR.to_string(),"description":"Content API and authenticated hosted server operations. External integrations use scoped content credentials or MCP OAuth."},
        "servers":[{"url":"/"}],
        "paths":paths,
        "components":{
            "schemas":generator.take_definitions(true),
            "securitySchemes":{
                "ownerSession":{"type":"apiKey","in":"cookie","name":"__Host-lapis_session","description":"Sign in with the Lapis browser. Browser mutations also need the session CSRF token."},
                "operatorToken":{"type":"http","scheme":"bearer","description":"Configured owner-wide server token."},
                "contentToken":{"type":"http","scheme":"bearer","description":"Scoped personal or temporary content token."},
                "syncDiscoveryToken":{"type":"http","scheme":"bearer","description":"Scoped native account discovery credential."},
                "oauthClientBasic":{"type":"http","scheme":"basic","description":"OAuth client registration may require HTTP Basic, form credentials, or public-client PKCE."},
                "mcpOAuth":{"type":"oauth2","flows":{"authorizationCode":{"authorizationUrl":"/oauth/authorize","tokenUrl":"/oauth/token","scopes":scopes}}}
            }
        },
        "x-lapis-plugin-operations":operations
    })
}

fn operation(entry: &Endpoint, path: &str, generator: &mut SchemaGenerator) -> Value {
    let modifying = !matches!(entry.method, "get" | "head");
    let (tag, security) = match entry.auth {
        Auth::Public | Auth::Login => ("Public and sign-in", json!([])),
        Auth::Owner => (
            "Server owner API",
            json!([{"ownerSession":[]},{"operatorToken":[]} ]),
        ),
        Auth::Browser => ("Browser session", json!([{"ownerSession":[]} ])),
        Auth::BrowserNavigation => ("Documentation", json!([{"ownerSession":[]} ])),
        Auth::Content => ("Content API", json!([{"contentToken":[]} ])),
        Auth::Mcp => (
            "Model Context Protocol",
            json!([{"contentToken":[]},{"mcpOAuth":["mcp:read"]} ]),
        ),
        Auth::OAuthClient => ("OAuth authorization", json!([{},{"oauthClientBasic":[]} ])),
        Auth::Sync if path.starts_with("/api/v3/sync/discovery/") => {
            ("Native sync", json!([{"syncDiscoveryToken":[]}]))
        }
        Auth::Sync => ("Native sync", json!([])),
    };
    let tag = if path.starts_with("/oauth/") || path.starts_with("/.well-known/oauth-") {
        "OAuth authorization"
    } else {
        tag
    };
    let mut params = parameters(path, modifying);
    if matches!(
        entry.auth,
        Auth::Owner | Auth::Browser | Auth::Login | Auth::Sync
    ) && path.starts_with("/api/v3/")
    {
        params.push(parameter(
            "x-lapis-protocol",
            "header",
            true,
            "Hosted browser protocol major",
            json!({"type":"string","const":"3","default":"3"}),
        ));
    }
    if modifying && matches!(entry.auth, Auth::Owner | Auth::Browser) {
        params.push(parameter(
            "x-lapis-csrf",
            "header",
            false,
            "Required for browser-session mutations. Swagger UI sends the current session value.",
            json!({"type":"string"}),
        ));
    }
    if matches!(entry.auth, Auth::Content) {
        params.push(parameter(
            "x-lapis-content-api",
            "header",
            true,
            "Content API major",
            json!({"type":"string","const":"1","default":"1"}),
        ));
        params.push(parameter(
            "x-lapis-request-id",
            "header",
            false,
            "Request correlation UUID",
            json!({"type":"string","format":"uuid"}),
        ));
    }
    let mut responses = Map::new();
    let mut success = json!({"description":if entry.success == 405 { "Unsupported transport method" } else { "Successful response" }});
    if let Some(output) = entry.output {
        success["content"] = body(output, generator);
    }
    if matches!(entry.method, "get" | "put")
        && (path.contains("/files/{path}") || path.ends_with("/note"))
    {
        success["headers"] = json!({"ETag":{"description":"Strong saved content version","schema":{"type":"string"}}});
    }
    if path == "/mcp" && entry.method == "post" {
        success["content"]["text/event-stream"] = json!({"schema":{"type":"string"}});
        responses.insert(
            "202".into(),
            json!({"description":"MCP notification accepted"}),
        );
    }
    responses.insert(entry.success.to_string(), success);
    for (code, description) in [
        (400, "Invalid request"),
        (401, "Missing or invalid authentication"),
        (403, "Permission denied"),
        (404, "Resource unavailable"),
        (409, "Conflict or protocol mismatch"),
        (413, "Request too large"),
        (503, "Unavailable or busy"),
    ] {
        let error_schema = match entry.auth {
            Auth::Content | Auth::Mcp => schema::<ContentReply>(generator),
            Auth::OAuthClient | Auth::Public if path.starts_with("/oauth/") => {
                json!({"type":"object","required":["error"],"properties":{"error":{"type":"string"}}})
            }
            _ => schema::<ServiceError>(generator),
        };
        responses.entry(code.to_string()).or_insert(json!({"description":description,"content":{"application/json":{"schema":error_schema}}}));
    }
    if entry.success == 303 {
        responses.get_mut("303").unwrap()["headers"] =
            json!({"Location":{"schema":{"type":"string","format":"uri-reference"}}});
    }
    let mut result = json!({"summary":entry.summary,"tags":[tag],"security":security,"parameters":params,"responses":responses});
    if let Some(input) = entry.input {
        result["requestBody"] = json!({"required":true,"content":body(input,generator)});
    }
    result
}

fn body(body: Body, generator: &mut SchemaGenerator) -> Value {
    json!({body.media: {"schema":(body.schema)(generator)}})
}

fn parameter(
    name: &str,
    location: &str,
    required: bool,
    description: &str,
    schema: Value,
) -> Value {
    json!({"name":name,"in":location,"required":required,"description":description,"schema":schema})
}

fn parameters(path: &str, modifying: bool) -> Vec<Value> {
    let mut values: Vec<Value> = path
        .split('{')
        .skip(1)
        .filter_map(|part| part.split_once('}'))
        .map(|(name, _)| {
            parameter(
                name,
                "path",
                true,
                "Path component",
                json!({"type":"string"}),
            )
        })
        .collect();
    if path.ends_with("/note") {
        values.push(parameter(
            "path",
            "query",
            true,
            "Relative note path",
            json!({"type":"string"}),
        ));
    }
    if path.starts_with("/api/content/v1/")
        && (path.ends_with("/vaults") || path.ends_with("/notes"))
    {
        values.push(parameter(
            "cursor",
            "query",
            false,
            "Continuation returned by an earlier request",
            json!({"type":"string"}),
        ));
        values.push(parameter(
            "limit",
            "query",
            false,
            "Maximum returned entries",
            json!({"type":"integer","minimum":1,"maximum":100,"default":50}),
        ));
        if path.ends_with("/notes") {
            values.push(parameter(
                "directory",
                "query",
                false,
                "Relative directory to list",
                json!({"type":"string"}),
            ));
        }
    }
    if path.contains("/files/{path}") && modifying {
        for name in ["If-Match", "If-None-Match"] {
            values.push(parameter(
                name,
                "header",
                false,
                "Optional write precondition",
                json!({"type":"string"}),
            ));
        }
    }
    if path == "/mcp" {
        values.push(parameter(
            "MCP-Protocol-Version",
            "header",
            false,
            "Negotiated MCP protocol revision",
            json!({"type":"string"}),
        ));
        for name in ["Mcp-Method", "Mcp-Name"] {
            values.push(parameter(
                name,
                "header",
                false,
                "Required when applicable to negotiated per-request protocol",
                json!({"type":"string"}),
            ));
        }
    }
    if path == "/oauth/authorize" {
        for name in [
            "client_id",
            "redirect_uri",
            "response_type",
            "resource",
            "code_challenge",
            "code_challenge_method",
            "scope",
            "state",
        ] {
            values.push(parameter(
                name,
                "query",
                !matches!(name, "state" | "scope"),
                "OAuth authorization parameter",
                json!({"type":"string"}),
            ));
        }
    }
    values
}
