use lapis_server::{
    config::{Config, VaultConfig},
    control::Control,
    host::Host,
    http::{self, Authentication},
};
use reqwest::{Client, RequestBuilder};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

pub const OWNER: &str = "content-owner-test-token-0123456789abcdef";
pub const ORIGIN: &str = "https://notes.example";
pub const SAVED: &str = "# Saved 🪨\r\n";
pub struct Server {
    root: Option<tempfile::TempDir>,
    pub ids: [Uuid; 2],
    pub host: Arc<Host>,
    pub client: Client,
    pub base: String,
    pub origin: String,
    control: Option<Arc<Control>>,
    server: Option<tokio::task::JoinHandle<()>>,
}
impl Server {
    pub async fn start(persistent: bool, origin: &str) -> Self {
        Self::open(
            tempfile::tempdir().unwrap(),
            [Uuid::new_v4(), Uuid::new_v4()],
            persistent,
            origin,
        )
        .await
    }
    async fn open(root: tempfile::TempDir, ids: [Uuid; 2], persistent: bool, origin: &str) -> Self {
        let mut vaults = Vec::new();
        for (i, id) in ids.into_iter().enumerate() {
            let path = root.path().join(format!("vault-{i}"));
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("note.md"), SAVED).unwrap();
            vaults.push(VaultConfig {
                id,
                name: format!("Notes {i}"),
                root: path,
            });
        }
        let host = Arc::new(Host::new(Config { vaults }).unwrap());
        let profile = root.path().join("profile");
        let control = if persistent {
            let control = Arc::new(Control::open(&profile).await.unwrap());
            control
                .initialize("owner", crate::credentials::HASH, crate::credentials::NOW)
                .await
                .unwrap();
            host.enable_management(control.clone(), &profile)
                .await
                .unwrap();
            http::restore_content_settings(host.clone(), control.clone(), origin)
                .await
                .unwrap();
            Some(control)
        } else {
            None
        };
        let auth = match &control {
            Some(control) => Authentication::browser(control.clone(), Some(OWNER), false).unwrap(),
            None => Authentication::token(OWNER).unwrap(),
        };
        let router = http::router(host.clone(), auth, origin).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = Some(tokio::spawn(async move {
            axum::serve(
                listener,
                router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
            .unwrap();
        }));
        Self {
            root: Some(root),
            ids,
            host,
            client: Client::new(),
            base,
            origin: origin.into(),
            control,
            server,
        }
    }
    pub fn owner(&self, method: reqwest::Method, path: &str) -> RequestBuilder {
        self.client
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(OWNER)
            .header("x-lapis-protocol", "3")
    }
    pub async fn status(&self) -> Value {
        self.owner(reqwest::Method::GET, "/api/v3/content-access")
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    pub async fn enable(&self, enabled: bool) {
        let response = self
            .owner(reqwest::Method::POST, "/api/v3/content-access")
            .json(&json!({"enabled":enabled}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }
    pub async fn issue(&self, vaults: &[Uuid]) -> Value {
        let response = self
            .owner(reqwest::Method::POST, "/api/v3/content-access/grants")
            .json(&json!({"label":"Test client", "vault_ids":vaults,"lifetime_seconds":60}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
        assert_eq!(response.headers()["cache-control"], "no-store");
        response.json().await.unwrap()
    }
    pub fn external(&self, path: &str, token: &str) -> RequestBuilder {
        self.client
            .get(format!("{}{path}", self.base))
            .bearer_auth(token)
            .header("host", self.origin.split_once("://").unwrap().1)
            .header("x-lapis-content-api", "1")
    }
    pub fn mcp(&self, token: &str, vault: Uuid) -> RequestBuilder {
        self.client.post(format!("{}/mcp", self.base)).bearer_auth(token)
            .header("host", self.origin.split_once("://").unwrap().1)
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2026-07-28").header("mcp-method", "tools/call").header("mcp-name", "read_note")
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
                "name":"read_note","arguments":{"vault_id":vault,"path":"note.md"},
                "_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}
            }}))
    }
    pub async fn close_store(&self) {
        self.control.as_ref().unwrap().close().await.unwrap();
    }
    pub async fn finish(&mut self) {
        if let Some(server) = self.server.take() {
            server.abort();
            let _ = server.await;
        }
        let host = self.host.clone();
        tokio::task::spawn_blocking(move || host.shutdown())
            .await
            .unwrap()
            .unwrap();
        if let Some(control) = &self.control {
            control.close().await.unwrap();
        }
    }
    pub async fn restart(mut self) -> Self {
        self.finish().await;
        let root = self.root.take().unwrap();
        Self::open(root, self.ids, self.control.is_some(), &self.origin).await
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(server) = &self.server {
            server.abort();
        }
    }
}
