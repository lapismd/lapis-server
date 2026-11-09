use lapis_app_core::services::{
    Runtime,
    content::{ContentGrants, ContentVault},
};
use reqwest::Client;
use serde_json::{Value, json};
use std::{fs, sync::Arc, time::Duration};
use uuid::Uuid;

pub const SAVED: &str = "# Saved 🪨\r\n";
pub struct Fixture {
    pub runtime: Runtime,
    pub root: tempfile::TempDir,
    pub vault: Uuid,
    pub grants: Arc<ContentGrants>,
    pub grant: Uuid,
    pub token: String,
    pub url: String,
    pub client: Client,
    pub cancellation: tokio_util::sync::CancellationToken,
    server: tokio::task::JoinHandle<()>,
}
impl Fixture {
    pub async fn start() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a note.md"), SAVED).unwrap();
        fs::write(root.path().join("b.md"), "second").unwrap();
        let runtime = Runtime::default();
        let vault = Uuid::new_v4();
        let admitted = runtime.admit(vault, root.path()).unwrap();
        let grants = Arc::new(ContentGrants::default());
        let issued = grants
            .issue_read_only(
                vec![ContentVault {
                    name: "Personal".into(),
                    client: admitted.attach().unwrap(),
                }],
                Duration::from_secs(60),
            )
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let boundary = lapis_content_http::Boundary::new(grants.clone(), &origin).unwrap();
        let cancellation = tokio_util::sync::CancellationToken::new();
        let app = lapis_content_mcp::router_with_boundary(boundary.clone(), cancellation.clone())
            .unwrap()
            .merge(lapis_content_http::router_with_boundary(boundary));
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            runtime,
            root,
            vault,
            grants,
            grant: issued.grant_id,
            token: issued.token.expose_secret().into(),
            url: format!("{origin}/mcp"),
            client: Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            cancellation,
            server,
        }
    }
    pub fn request(&self) -> reqwest::RequestBuilder {
        self.client
            .post(&self.url)
            .bearer_auth(&self.token)
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2026-07-28")
    }
    pub async fn rpc(&self, method: &str, params: Value) -> Value {
        let response = self.rpc_request(method, params).send().await.unwrap();
        let status = response.status();
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        assert!(!response.headers().contains_key("mcp-session-id"));
        let body = response.text().await.unwrap();
        assert_eq!(status, 200, "{body}");
        serde_json::from_str(&body).unwrap()
    }
    pub fn rpc_request(&self, method: &str, params: Value) -> reqwest::RequestBuilder {
        let mut request = self.request().header("mcp-method", method);
        if let Some(name) = params
            .get("name")
            .or_else(|| params.get("uri"))
            .and_then(Value::as_str)
        {
            request = request.header("mcp-name", name);
        }
        request.json(&message(method, params))
    }
    pub async fn tool(&self, name: &str, arguments: Value) -> Value {
        self.rpc("tools/call", json!({"name":name,"arguments":arguments}))
            .await
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
        self.grants.shutdown().unwrap();
    }
}
pub fn message(method: &str, mut params: Value) -> Value {
    params["_meta"] = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28",
        "io.modelcontextprotocol/clientCapabilities":{}});
    json!({"jsonrpc":"2.0", "id":1, "method":method, "params":params})
}
