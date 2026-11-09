use super::mcp_support::*;
use axum::{
    Router,
    body::Body,
    extract::Request,
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
};
use rmcp::{
    ClientLifecycleMode, ClientServiceExt, model::CallToolRequestParams,
    transport::async_rw::AsyncRwTransport,
};
use serde_json::json;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

#[tokio::test]
async fn startup_failures_are_sanitized_and_never_consume_protocol_stdin() {
    let f = Fixture::start().await;
    for flag in ["--json", "--token-stdin"] {
        let mut child = f.command().arg(flag).spawn().unwrap();
        let held_input = child.stdin.take().unwrap();
        let result = finish(child).await;
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains(&f.token));
        drop(held_input);
    }
    f.grants.revoke(f.grant).unwrap();
    let result = finish(f.command().spawn().unwrap()).await;
    assert_eq!(result.status.code(), Some(3));
    assert!(result.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&result.stderr).contains(&f.token));
}

#[tokio::test]
async fn malformed_remote_responses_and_redirects_do_not_expose_or_forward_credentials() {
    let f = Fixture::start().await;
    let hits = Arc::new(AtomicUsize::new(0));
    let count = hits.clone();
    let (other, other_server) = serve(Router::new().fallback(move || {
        count.fetch_add(1, Ordering::SeqCst);
        async { "wrong destination" }
    }))
    .await;
    for redirect in [false, true] {
        let body = f.token.clone();
        let destination = other.clone();
        let (base, server) = serve(Router::new().fallback(move || {
            let body = body.clone();
            let destination = destination.clone();
            async move {
                if redirect {
                    Response::builder()
                        .status(StatusCode::FOUND)
                        .header("location", destination)
                        .body(Body::empty())
                        .unwrap()
                } else {
                    Response::builder()
                        .header("content-type", "application/json")
                        .body(Body::from(body))
                        .unwrap()
                }
            }
        }))
        .await;
        let result = finish(
            command(&base, &f.root.path().join("token"))
                .spawn()
                .unwrap(),
        )
        .await;
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains(&f.token));
        server.abort();
    }
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    other_server.abort();
}

#[tokio::test]
async fn a_timed_out_tool_is_not_replayed_and_the_bridge_remains_usable() {
    let f = Fixture::start().await;
    let hits = Arc::new(AtomicUsize::new(0));
    let count = hits.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = lapis_content_mcp::router(f.grants.clone(), &base)
        .unwrap()
        .layer(middleware::from_fn(move |request: Request, next: Next| {
            let count = count.clone();
            async move {
                if request
                    .headers()
                    .get("mcp-method")
                    .is_some_and(|h| h == "tools/call")
                {
                    count.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
                next.run(request).await
            }
        }));
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut child = command(&base, &f.root.path().join("token"))
        .spawn()
        .unwrap();
    let transport =
        AsyncRwTransport::new_client(child.stdout.take().unwrap(), child.stdin.take().unwrap());
    let client = ().serve_with_lifecycle(transport, ClientLifecycleMode::Initialize).await.unwrap();
    let error = client
        .call_tool(
            CallToolRequestParams::new("capabilities")
                .with_arguments(json!({}).as_object().unwrap().clone()),
        )
        .await
        .unwrap_err();
    let rmcp::ServiceError::McpError(error) = error else {
        panic!()
    };
    assert_eq!(error.data.unwrap()["code"], "UNAVAILABLE");
    assert_eq!(client.list_all_tools().await.unwrap().len(), 4);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    client.cancel().await.unwrap();
    assert!(finish(child).await.status.success());
    server.abort();
}

async fn serve(app: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (base, server)
}
