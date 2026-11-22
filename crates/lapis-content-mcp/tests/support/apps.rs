use crate::{operations::SavedNote, support::Fixture};
use lapis_app_core::services::plugin_operations::{OperationProvider, OperationRegistry};
use lapis_content_mcp::{AppRegistry, AppResource};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

const NOTE: &str = "ui://lapis/notes/read-v1";
const PLUGIN: &str = "ui://lapis/example-plugin/read-v1";
const MIME: &str = "text/html;profile=mcp-app";

fn resource(uri: &str) -> AppResource {
    AppResource {
        uri: uri.into(),
        title: "Fixture view".into(),
        html: "<main>Fixture view</main>".into(),
    }
}

fn registry() -> AppRegistry {
    let mut apps = AppRegistry::default();
    assert!(apps.register_builtin("unknown", resource(NOTE)).is_err());
    apps.register_builtin("read_note", resource(NOTE)).unwrap();
    assert!(
        apps.register_builtin("read_note", resource(PLUGIN))
            .is_err()
    );
    let descriptors = SavedNote.descriptors();
    assert!(
        apps.register_provider(
            &descriptors,
            "example-plugin",
            "unknown",
            &[],
            resource(PLUGIN)
        )
        .is_err()
    );
    apps.register_provider(
        &descriptors,
        "example-plugin",
        "saved-note",
        &["saved-note"],
        resource(PLUGIN),
    )
    .unwrap();
    apps
}

async fn request(f: &Fixture, method: &str, mut params: Value) -> Value {
    let name = params
        .get("name")
        .or_else(|| params.get("uri"))
        .and_then(Value::as_str);
    let mut request = f.request().header("mcp-method", method);
    if let Some(name) = name {
        request = request.header("mcp-name", name);
    }
    params["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientCapabilities": {
            "extensions": { "io.modelcontextprotocol/ui": { "mimeTypes": [MIME] } }
        }
    });
    let response = request
        .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    response.json().await.unwrap()
}

#[tokio::test]
async fn app_discovery_uses_capability_and_catalog_without_affecting_fallback() {
    let mut operations = OperationRegistry::default();
    operations.register(Arc::new(SavedNote)).unwrap();
    let f = Fixture::with_apps(
        Arc::new(operations),
        &["example-plugin:read".into()],
        registry(),
    )
    .await;
    let plain = f.rpc("tools/list", json!({})).await;
    assert!(
        plain["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|tool| tool["_meta"].is_null())
    );
    assert_eq!(
        f.rpc("resources/list", json!({})).await["result"]["resources"],
        json!([])
    );
    let tools = request(&f, "tools/list", json!({})).await;
    for (name, uri) in [("read_note", NOTE), ("example-plugin_saved-note", PLUGIN)] {
        let tool = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap();
        assert_eq!(tool["_meta"]["ui"]["resourceUri"], uri);
    }
    let listed = request(&f, "resources/list", json!({})).await;
    assert_eq!(listed["result"]["resources"].as_array().unwrap().len(), 2);
    let asset = request(&f, "resources/read", json!({"uri":PLUGIN})).await;
    let content = &asset["result"]["contents"][0];
    assert_eq!(content["mimeType"], MIME);
    assert_eq!(content["text"], "<main>Fixture view</main>");
    assert_eq!(content["_meta"]["ui"]["permissions"], json!({}));
    assert_eq!(content["_meta"]["ui"]["csp"]["connectDomains"], json!([]));
    let result = request(
        &f,
        "tools/call",
        json!({
            "name": "example-plugin_saved-note", "arguments": {"vault_id": f.vault}
        }),
    )
    .await;
    assert_eq!(
        result["result"]["_meta"]["io.lapismd/availableTools"],
        json!(["example-plugin_saved-note"])
    );
    assert_eq!(
        result["result"]["structuredContent"]["outcome"]["value"],
        super::support::SAVED
    );
    assert_eq!(result["result"]["content"][0]["type"], "text");
    let denied = request(
        &f,
        "tools/call",
        json!({
            "name": "example-plugin_saved-note", "arguments": {"vault_id": Uuid::new_v4()}
        }),
    )
    .await;
    assert!(denied["result"]["_meta"].is_null());
    assert_eq!(
        denied["result"]["structuredContent"]["outcome"]["error"]["code"],
        "PERMISSION_DENIED"
    );
    f.grants.revoke(f.grant).unwrap();
    let response = f
        .rpc_request("resources/read", json!({"uri": NOTE}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn knowing_a_provider_app_uri_does_not_grant_access() {
    let mut operations = OperationRegistry::default();
    operations.register(Arc::new(SavedNote)).unwrap();
    let f = Fixture::with_apps(Arc::new(operations), &[], registry()).await;
    let listed = request(&f, "resources/list", json!({})).await;
    assert_eq!(listed["result"]["resources"].as_array().unwrap().len(), 1);
    assert_eq!(listed["result"]["resources"][0]["uri"], NOTE);
    let refused = request(&f, "resources/read", json!({"uri":PLUGIN})).await;
    assert_eq!(refused["error"]["code"], -32602);
    assert_eq!(refused["error"]["message"], "MCP App resource not found");
}
