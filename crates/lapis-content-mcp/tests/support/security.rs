use super::support::*;
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn credentials_origins_and_shutdown_fence_every_request() {
    let mut f = Fixture::start().await;
    for (name, value, status) in [
        ("origin", "https://evil.example", 403),
        ("host", "evil.example", 403),
        ("authorization", "Bearer owner-token", 401),
        ("mcp-session-id", "cannot-supply-authority", 400),
        ("mcp-protocol-version", "2026-07-28", 400),
    ] {
        let response = f
            .rpc_request("tools/list", json!({}))
            .header(name, value)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{name}");
        let body = response.text().await.unwrap();
        assert!(!body.contains(&f.token));
        assert!(!body.contains(f.root.path().to_str().unwrap()));
    }
    let denied = f
        .client
        .post(&f.url)
        .body("malformed json")
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 401);
    assert!(denied.headers().contains_key("www-authenticate"));
    f.grants.revoke(f.grant).unwrap();
    assert_eq!(
        f.rpc_request("tools/list", json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let expired = f
        .grants
        .issue_read_only(vec![], Duration::from_millis(1))
        .unwrap();
    f.token = expired.token.expose_secret().into();
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert_eq!(
        f.rpc_request("server/discover", json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    f.cancellation.cancel();
    assert_eq!(
        f.rpc_request("tools/list", json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        503
    );
}

#[tokio::test]
async fn protocol_metadata_and_routing_headers_are_checked_before_dispatch() {
    let f = Fixture::start().await;
    let body = message("tools/list", json!({}));
    let cases = [
        f.request().json(&body), // missing required method header
        f.request().header("mcp-method", "tools/call").json(&body),
        f.request()
            .header("mcp-method", "tools/list")
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}})),
        f.request().header("mcp-method", "tools/list").json(&{
            let mut legacy = body.clone();
            legacy["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"] =
                json!("2025-11-25");
            legacy
        }),
        f.request().header("mcp-method", "tools/list").json(&{
            let mut absent = body.clone();
            absent["params"]["_meta"]
                .as_object_mut()
                .unwrap()
                .remove("io.modelcontextprotocol/clientCapabilities");
            absent
        }),
        f.rpc_request(
            "tools/call",
            json!({"name":"read_note","arguments":{"vault_id":f.vault,"path":"a note.md"}}),
        )
        .header("mcp-name", "capabilities"),
    ];
    for request in cases {
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), 400, "{}", response.text().await.unwrap());
    }
    // The SDK rejects JSON-RPC batches as an unsupported body representation.
    let batch = f
        .request()
        .header("mcp-method", "tools/list")
        .json(&vec![body.clone(), body])
        .send()
        .await
        .unwrap();
    assert_eq!(batch.status(), 415);
    let legacy = f
        .client
        .post(&f.url)
        .bearer_auth(&f.token)
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-11-25")
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}))
        .send()
        .await
        .unwrap();
    assert_eq!(legacy.status(), 400);
    for method in [reqwest::Method::GET, reqwest::Method::DELETE] {
        let response = f
            .client
            .request(method, &f.url)
            .bearer_auth(&f.token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 405);
    }
}

#[tokio::test]
async fn bounded_streaming_bodies_and_metadata_do_not_reach_content_handlers() {
    let f = Fixture::start().await;
    let body = vec![b' '; lapis_content_mcp::MAX_REQUEST_BYTES + 1];
    for streaming in [false, true] {
        let request = f
            .request()
            .header("mcp-method", "tools/list")
            .header("content-type", "application/json");
        let request = if streaming {
            request.body(reqwest::Body::wrap_stream(futures_util::stream::iter(
                vec![Ok::<_, std::io::Error>(body.clone())],
            )))
        } else {
            request.body(body.clone())
        };
        assert_eq!(request.send().await.unwrap().status(), 413);
    }
    let response = f
        .rpc_request("tools/list", json!({}))
        .header("x-padding", "x".repeat(8192))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
    let response = f
        .client
        .post(format!("{}?padding={}", f.url, "x".repeat(16 * 1024)))
        .bearer_auth(&f.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
}
