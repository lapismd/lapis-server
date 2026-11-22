use crate::support;
use lapis_app_core::services::{Client, plugin_operations::*};
use lapis_app_protocol::{content::*, operations::*};
use rmcp::{
    ClientLifecycleMode, ClientServiceExt,
    model::{CallToolRequestParams, ProtocolVersion},
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;

struct SavedNote;
impl OperationProvider for SavedNote {
    fn descriptors(&self) -> Vec<OperationDescriptor> {
        vec![OperationDescriptor {
            plugin_id: "example-plugin".into(),
            id: "saved-note".into(),
            version: 1,
            description: "Read a saved fixture note".into(),
            permission: "example-plugin:read".into(),
            effect: ContentEffect::ReadOnly,
            input_schema: json!({"type":"object","properties":{},"additionalProperties":false}),
            output_schema: json!({"type":"string"}),
            help: OperationHelp {
                command: vec!["example-plugin".into(), "saved-note".into()],
                summary: "Read saved fixture".into(),
                markdown: "Read a saved fixture note.".into(),
                examples: vec![],
                keywords: vec![],
                arguments: vec![],
            },
        }]
    }
    fn available(&self, _: &Client) -> Result<bool, ContentError> {
        Ok(true)
    }
    fn invoke(
        &self,
        client: &Client,
        _: &str,
        _: &OperationRequest,
    ) -> Result<Value, ContentError> {
        client
            .read_text("a note.md")
            .map(|v| json!(v.text))
            .map_err(|_| failure(ContentErrorCode::Unavailable, "Saved note unavailable"))
    }
}

#[tokio::test]
async fn registered_provider_has_http_and_official_mcp_parity() {
    let mut registry = OperationRegistry::default();
    registry.register(Arc::new(SavedNote)).unwrap();
    let f = support::Fixture::with_operations(Arc::new(registry), &["example-plugin:read".into()])
        .await;
    let base = f.url.trim_end_matches("/mcp");
    let catalog: Value = f
        .client
        .get(format!(
            "{base}/api/content/v1/operations?vault_id={}",
            f.vault
        ))
        .bearer_auth(&f.token)
        .header("x-lapis-content-api", "1")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(catalog["operations"][0]["id"], "saved-note");
    let id = uuid::Uuid::new_v4();
    let refused = f
        .client
        .post(format!(
            "{base}/api/content/v1/vaults/{}/plugins/example-plugin/operations/saved-note",
            f.vault
        ))
        .bearer_auth("invalid")
        .header("x-lapis-content-api", "1")
        .header("x-lapis-request-id", id.to_string())
        .json(&json!({"protocol_version":1,"version":1,"request_id":id,"input":{}}))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 401);
    assert_eq!(refused.headers()["x-lapis-request-id"], id.to_string());
    let response = f
        .client
        .post(format!(
            "{base}/api/content/v1/vaults/{}/plugins/example-plugin/operations/saved-note",
            f.vault
        ))
        .bearer_auth(&f.token)
        .header("x-lapis-content-api", "1")
        .header("x-lapis-request-id", id.to_string())
        .json(&json!({"protocol_version":1,"version":1,"request_id":id,"input":{}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let http: Value = response.json().await.unwrap();
    assert_eq!(http["outcome"]["value"], support::SAVED);
    let transport = StreamableHttpClientTransport::with_client(
        f.client.clone(),
        StreamableHttpClientTransportConfig::with_uri(f.url.clone()).auth_header(f.token.clone()),
    );
    let client = ()
        .serve_with_lifecycle(
            transport,
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
        )
        .await
        .unwrap();
    let tools = client.list_all_tools().await.unwrap();
    assert!(tools.iter().any(|t| t.name == "example-plugin_saved-note"));
    let capabilities = client
        .call_tool(CallToolRequestParams::new("capabilities"))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    let operations = capabilities["outcome"]["value"]["capabilities"]["operations"]
        .as_array()
        .unwrap();
    assert_eq!(operations.len(), tools.len());
    for tool in &tools {
        assert!(operations.iter().any(|entry| entry["operation"] == tool.name.as_ref()));
    }
    let result = client
        .call_tool(
            CallToolRequestParams::new("example-plugin_saved-note")
                .with_arguments(json!({"vault_id":f.vault}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert_eq!(
        result.structured_content.unwrap()["outcome"]["value"],
        support::SAVED
    );
    f.grants.revoke(f.grant).unwrap();
    assert!(client.list_all_tools().await.is_err());
    client.cancel().await.unwrap();
}
