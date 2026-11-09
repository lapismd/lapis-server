#![cfg(unix)]
mod mcp_support;
#[path = "mcp_support/security.rs"]
mod security;

use mcp_support::*;
use rmcp::{
    ClientLifecycleMode, ClientServiceExt,
    model::{CallToolRequestParams, ProtocolVersion, ReadResourceRequestParams},
    transport::async_rw::AsyncRwTransport,
};
use serde_json::json;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn official_stdio_clients_bridge_both_lifecycles_to_remote_reads() {
    let f = Fixture::start().await;
    for (lifecycle, expected_version) in [
        (
            ClientLifecycleMode::Initialize,
            ProtocolVersion::V_2025_11_25,
        ),
        (
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
            ProtocolVersion::V_2026_07_28,
        ),
    ] {
        let mut child = f.command().spawn().unwrap();
        let transport =
            AsyncRwTransport::new_client(child.stdout.take().unwrap(), child.stdin.take().unwrap());
        let client = ().serve_with_lifecycle(transport, lifecycle).await.unwrap();
        assert_eq!(
            client.peer_info().unwrap().protocol_version,
            expected_version
        );
        let tools = client.list_all_tools().await.unwrap();
        assert_eq!(tools.len(), 4);
        assert_eq!(client.list_all_resource_templates().await.unwrap().len(), 1);
        assert!(client.list_all_resources().await.unwrap().is_empty());
        let note = client
            .call_tool(
                CallToolRequestParams::new("read_note").with_arguments(
                    json!({"vault_id":f.vault,"path":"note.md"})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
            )
            .await
            .unwrap();
        assert_eq!(
            note.structured_content.unwrap()["outcome"]["value"]["text"],
            SAVED
        );
        let resource = client
            .read_resource(ReadResourceRequestParams::new(format!(
                "lapis://notes/{}/note.md",
                f.vault
            )))
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(resource).unwrap()["contents"][0]["text"],
            SAVED
        );
        client.cancel().await.unwrap();
        let output = finish(child).await;
        assert!(output.status.success(), "{:?}", output.stderr);
        assert!(output.stderr.is_empty());
    }
}

#[tokio::test]
async fn revocation_and_foreign_vault_failures_cross_the_bridge_without_cached_reads() {
    let f = Fixture::start().await;
    let mut child = f.command().spawn().unwrap();
    let transport =
        AsyncRwTransport::new_client(child.stdout.take().unwrap(), child.stdin.take().unwrap());
    let client = ().serve_with_lifecycle(transport, ClientLifecycleMode::Initialize).await.unwrap();
    client
        .peer()
        .set_response_cache_config(rmcp::ClientCacheConfig::disabled())
        .await;
    assert_eq!(client.list_all_tools().await.unwrap().len(), 4);
    let note = client
        .call_tool(
            CallToolRequestParams::new("read_note").with_arguments(
                json!({"vault_id":uuid::Uuid::new_v4(),"path":"note.md"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(note.is_error, Some(true));
    assert_eq!(
        note.structured_content.unwrap()["outcome"]["error"]["code"],
        "PERMISSION_DENIED"
    );
    f.grants.revoke(f.grant).unwrap();
    let rmcp::ServiceError::McpError(error) = client.list_all_tools().await.unwrap_err() else {
        panic!()
    };
    assert_eq!(error.data.unwrap()["code"], "UNAUTHENTICATED");
    client.cancel().await.unwrap();
    assert!(finish(child).await.status.success());
}

#[tokio::test]
async fn closed_stdout_exits_and_oversized_lines_are_discarded_without_losing_the_next_request() {
    let f = Fixture::start().await;
    for close_stdout in [true, false] {
        let mut child = f.command().spawn().unwrap();
        let mut input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        input.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test\",\"version\":\"1\"}}}\n").await.unwrap();
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(3), output.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();
        assert!(
            serde_json::from_str::<serde_json::Value>(&line)
                .unwrap()
                .get("result")
                .is_some()
        );
        input
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
            .await
            .unwrap();
        if close_stdout {
            drop(output);
            input
                .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n")
                .await
                .unwrap();
        } else {
            input.write_all(&vec![b'x'; 64 * 1024 + 1]).await.unwrap();
            input
                .write_all(b"\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n")
                .await
                .unwrap();
            line.clear();
            tokio::time::timeout(Duration::from_secs(3), output.read_line(&mut line))
                .await
                .unwrap()
                .unwrap();
            let result: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(result["id"], 2);
            assert_eq!(result["result"]["tools"].as_array().unwrap().len(), 4);
        }
        // Keep the write end open: waiting for EOF would deadlock an MCP client.
        let held_input = if close_stdout {
            Some(input)
        } else {
            drop(input);
            None
        };
        let result = finish(child).await;
        assert!(!String::from_utf8_lossy(&result.stderr).contains(&f.token));
        drop(held_input);
    }
}
