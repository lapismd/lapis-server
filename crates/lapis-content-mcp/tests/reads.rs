#[path = "support/security.rs"]
mod security;
mod support;

use lapis_app_core::services::content::ContentVault;
use rmcp::{
    ClientLifecycleMode, ClientServiceExt,
    model::{CallToolRequestParams, ProtocolVersion, ReadResourceRequestParams},
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::json;
use std::{fs, time::Duration};
use support::*;
use uuid::Uuid;

#[tokio::test]
async fn discovery_requires_a_credential_and_only_advertises_reads() {
    let f = Fixture::start().await;
    assert_eq!(
        f.client
            .post(&f.url)
            .json(&message("tools/list", json!({})))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let result = f.rpc("tools/list", json!({})).await;
    let tools = result["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 4);
    assert!(!tools.iter().any(|t| t["name"] == "create_note"));
    let capabilities = f.tool("capabilities", json!({})).await;
    let operations = capabilities["result"]["structuredContent"]["outcome"]["value"]["capabilities"]["operations"].as_array().unwrap();
    for tool in tools {
        assert!(operations.iter().any(|op| op["operation"] == tool["name"]));
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(tool["inputSchema"]["type"], "object");
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert!(tool["outputSchema"]["properties"]["outcome"].is_object());
        assert!(!tool["inputSchema"].to_string().contains("$ref"));
        assert!(tool["inputSchema"]["properties"].get("operation").is_none());
    }
    let list = f.rpc("resources/list", json!({})).await;
    assert_eq!(list["result"]["resources"], json!([]));
    let templates = f.rpc("resources/templates/list", json!({})).await;
    assert_eq!(
        templates["result"]["resourceTemplates"][0]["uriTemplate"],
        "lapis://notes/{vault_id}/{path}"
    );
    let cursor = f.rpc("tools/list", json!({"cursor":"made-up"})).await;
    assert_eq!(cursor["error"]["code"], -32602);
}

#[tokio::test]
async fn note_tools_and_resources_preserve_saved_bytes_and_enforce_scope() {
    let f = Fixture::start().await;
    let note = f
        .rpc(
            "tools/call",
            json!({"name":"read_note", "arguments":{
                "vault_id": f.vault, "path":"a note.md"
            }}),
        )
        .await;
    assert_eq!(
        note["result"]["structuredContent"]["outcome"]["value"]["text"],
        SAVED
    );
    let resource = f
        .rpc(
            "resources/read",
            json!({"uri":format!("lapis://notes/{}/a%20note.md",f.vault)}),
        )
        .await;
    assert_eq!(resource["result"]["contents"][0]["text"], SAVED);
    let version = &note["result"]["structuredContent"]["outcome"]["value"]["version"];
    assert_eq!(
        &resource["result"]["contents"][0]["_meta"]["io.lapismd/contentVersion"],
        version
    );
    assert_eq!(resource["result"]["ttlMs"], 0);
    assert_eq!(resource["result"]["cacheScope"], "private");
    let http = f
        .client
        .get(format!(
            "{}/api/content/v1/vaults/{}/note?path=a%20note.md",
            f.url.trim_end_matches("/mcp"),
            f.vault
        ))
        .bearer_auth(&f.token)
        .header("x-lapis-content-api", "1")
        .send()
        .await
        .unwrap();
    assert_eq!(
        http.headers()["etag"],
        format!("\"{}\"", version.as_str().unwrap())
    );
    assert_eq!(http.text().await.unwrap(), SAVED);
    let foreign = Uuid::new_v4();
    let denied = f
        .tool("read_note", json!({"vault_id":foreign,"path":"a note.md"}))
        .await;
    tool_error(&denied, "PERMISSION_DENIED");
    let denied = f
        .rpc(
            "resources/read",
            json!({"uri":format!("lapis://notes/{foreign}/a%20note.md")}),
        )
        .await;
    assert_eq!(denied["error"]["data"]["code"], "PERMISSION_DENIED");
    for path in [
        "../secret.md",
        ".lapis/secret.md",
        "a%ZZ",
        "a%FF",
        "%2e%2e/secret.md",
        "/absolute.md",
    ] {
        let uri = format!("lapis://notes/{}/{path}", f.vault);
        assert!(
            f.rpc("resources/read", json!({"uri":uri}))
                .await
                .get("error")
                .is_some()
        );
    }
}

#[tokio::test]
async fn pagination_remains_bound_to_the_grant_and_the_listing_snapshot() {
    let mut f = Fixture::start().await;
    let vaults = f.tool("list_vaults", json!({"page":{"limit":1}})).await;
    assert_eq!(
        vaults["result"]["structuredContent"]["outcome"]["value"]["items"][0]["vault_id"],
        f.vault.to_string()
    );
    let page = f
        .tool("list_notes", json!({"vault_id":f.vault,"page":{"limit":1}}))
        .await;
    let cursor = page["result"]["structuredContent"]["outcome"]["value"]["next_cursor"].clone();
    assert!(cursor.is_string());
    let next = json!({"vault_id":f.vault,"page":{"limit":1,"cursor":cursor}});
    let page = f.tool("list_notes", next.clone()).await;
    assert_eq!(
        page["result"]["structuredContent"]["outcome"]["value"]["items"][0]["path"],
        "b.md"
    );
    let vault = f.runtime.admit(f.vault, f.root.path()).unwrap();
    let second = f
        .grants
        .issue_read_only(
            vec![ContentVault {
                name: "Personal".into(),
                client: vault.attach().unwrap(),
            }],
            Duration::from_secs(60),
        )
        .unwrap();
    let first = std::mem::replace(&mut f.token, second.token.expose_secret().into());
    tool_error(&f.tool("list_notes", next.clone()).await, "CONFLICT");
    f.token = first;
    fs::write(f.root.path().join("c.md"), "changed").unwrap();
    tool_error(&f.tool("list_notes", next).await, "CONFLICT");
}

#[tokio::test]
async fn official_sdk_discovers_and_reads_over_stateless_http() {
    let f = Fixture::start().await;
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
    assert_eq!(tools.len(), 4);
    let result = client
        .call_tool(
            CallToolRequestParams::new("read_note").with_arguments(
                json!({"vault_id":f.vault,"path":"a note.md"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        result.structured_content.unwrap()["outcome"]["value"]["text"],
        SAVED
    );
    let resource = client
        .read_resource(ReadResourceRequestParams::new(format!(
            "lapis://notes/{}/a%20note.md",
            f.vault
        )))
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(resource).unwrap()["contents"][0]["text"],
        SAVED
    );
    f.grants.revoke(f.grant).unwrap();
    assert!(client.list_all_tools().await.is_err());
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn arguments_and_write_tools_cannot_escape_the_read_catalog() {
    let f = Fixture::start().await;
    for (name, args) in [
        (
            "create_note",
            json!({"vault_id":f.vault,"path":"new.md","text":"no"}),
        ),
        ("capabilities", json!({"operation":"create_note"})),
        (
            "read_note",
            json!({"vault_id":f.vault,"path":"a note.md","extra":true}),
        ),
    ] {
        let error = f.tool(name, args).await;
        assert_eq!(error["error"]["code"], -32602);
    }
    tool_error(
        &f.tool("list_vaults", json!({"page":{"limit":0}})).await,
        "INVALID_REQUEST",
    );
    tool_error(
        &f.tool(
            "read_note",
            json!({"vault_id":f.vault,"path":"../secret.md"}),
        )
        .await,
        "INVALID_REQUEST",
    );
    tool_error(
        &f.tool("read_note", json!({"vault_id":f.vault,"path":"missing.md"}))
            .await,
        "NOT_FOUND",
    );
    fs::write(f.root.path().join("large.md"), "x".repeat(1024 * 1024 + 1)).unwrap();
    tool_error(
        &f.tool("read_note", json!({"vault_id":f.vault,"path":"large.md"}))
            .await,
        "TOO_LARGE",
    );
    assert!(!f.root.path().join("new.md").exists());
}

fn tool_error(reply: &serde_json::Value, code: &str) {
    assert_eq!(reply["result"]["isError"], true, "{reply}");
    assert_eq!(
        reply["result"]["structuredContent"]["outcome"]["error"]["code"],
        code
    );
    assert_eq!(
        reply["result"]["structuredContent"]["outcome"]["error"]["effect"],
        "not_started"
    );
}
