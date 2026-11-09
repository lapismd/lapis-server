use super::*;
use axum::body::{Body, Bytes, to_bytes};
use lapis_app_core::services::content::ContentGrants;
use serde_json::{Value, json};
use tower::ServiceExt;

fn fixture() -> (Router, Arc<ContentGrants>, Uuid, String, CancellationToken) {
    let grants = Arc::new(ContentGrants::default());
    let issued = grants
        .issue_read_only(vec![], Duration::from_secs(60))
        .unwrap();
    let boundary = Boundary::new(grants.clone(), "http://localhost:8787").unwrap();
    let cancellation = CancellationToken::new();
    (
        router(boundary, cancellation.clone()).unwrap(),
        grants,
        issued.grant_id,
        issued.token.expose_secret().into(),
        cancellation,
    )
}
fn message() -> String {
    json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{
        "io.modelcontextprotocol/protocolVersion":"2026-07-28",
        "io.modelcontextprotocol/clientCapabilities":{}
    }}})
    .to_string()
}
fn request(token: &str, body: Body) -> Request {
    Request::builder()
        .uri("/mcp")
        .method("POST")
        .header("host", "localhost:8787")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", "2026-07-28")
        .header("mcp-method", "tools/list")
        .body(body)
        .unwrap()
}
fn waiting_body() -> (
    Body,
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (started, observed) = tokio::sync::oneshot::channel();
    let (release, waiting) = tokio::sync::oneshot::channel();
    let body = Body::from_stream(futures_util::stream::once(async move {
        // Only the SDK can poll this stream, after middleware authentication.
        let _ = started.send(());
        let _ = waiting.await;
        Ok::<_, std::io::Error>(Bytes::from(message()))
    }));
    (body, observed, release)
}

#[tokio::test]
async fn revocation_after_http_preflight_prevents_mcp_dispatch() {
    let (app, grants, grant, token, _) = fixture();
    let (body, started, release) = waiting_body();
    let task = tokio::spawn(app.oneshot(request(&token, body)));
    tokio::time::timeout(Duration::from_secs(5), started)
        .await
        .unwrap()
        .unwrap();
    grants.revoke(grant).unwrap();
    release.send(()).unwrap();
    let response = task.await.unwrap().unwrap();
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(value["error"]["data"]["code"], "UNAUTHENTICATED");
    assert!(value.get("result").is_none());
}

#[tokio::test]
async fn request_budget_refuses_excess_bodies_and_recovers_after_cancellation() {
    let (app, grants, _, token, cancellation) = fixture();
    let mut tasks = Vec::new();
    let mut releases = Vec::new();
    for _ in 0..MAX_HTTP_REQUESTS {
        let (body, started, release) = waiting_body();
        tasks.push(tokio::spawn(app.clone().oneshot(request(&token, body))));
        releases.push(release);
        tokio::time::timeout(Duration::from_secs(5), started)
            .await
            .unwrap()
            .unwrap();
    }
    let response = app
        .clone()
        .oneshot(request(&token, Body::from(message())))
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
    let task = tasks.pop().unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let response = app
        .clone()
        .oneshot(request(&token, Body::from(message())))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    cancellation.cancel();
    for task in tasks {
        assert_eq!(task.await.unwrap().unwrap().status(), 503);
    }
    grants.shutdown().unwrap();
    drop(releases);
}

#[tokio::test]
async fn slow_request_body_times_out_and_releases_request_capacity() {
    let (app, grants, _, token, _) = fixture();
    let (body, started, _release) = waiting_body();
    let task = tokio::spawn(app.clone().oneshot(request(&token, body)));
    tokio::time::timeout(Duration::from_secs(5), started)
        .await
        .unwrap()
        .unwrap();
    let response = tokio::time::timeout(Duration::from_secs(16), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(response.status(), 503);
    assert_eq!(
        app.oneshot(request(&token, Body::from(message())))
            .await
            .unwrap()
            .status(),
        200
    );
    grants.shutdown().unwrap();
}
