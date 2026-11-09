use lapis_app_core::services::{
    Runtime,
    content::{ContentGrants, ContentVault},
};
use std::{fs, os::unix::fs::PermissionsExt, process::Stdio, sync::Arc, time::Duration};
use tokio::process::{Child, Command};
use uuid::Uuid;

pub const SAVED: &str = "# Exact 🪨\r\nno final newline";
pub struct Fixture {
    pub root: tempfile::TempDir,
    pub vault: Uuid,
    pub grants: Arc<ContentGrants>,
    pub grant: Uuid,
    pub token: String,
    pub base: String,
    server: tokio::task::JoinHandle<()>,
    _runtime: Runtime,
}
impl Fixture {
    pub async fn start() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("note.md"), SAVED).unwrap();
        let runtime = Runtime::default();
        let vault = Uuid::new_v4();
        let native = runtime.admit(vault, root.path()).unwrap();
        let grants = Arc::new(ContentGrants::default());
        let issued = grants
            .issue_read_only(
                vec![ContentVault {
                    name: "Personal".into(),
                    client: native.attach().unwrap(),
                }],
                Duration::from_secs(60),
            )
            .unwrap();
        let token = issued.token.expose_secret().to_owned();
        let token_path = root.path().join("token");
        fs::write(&token_path, &token).unwrap();
        fs::set_permissions(&token_path, fs::Permissions::from_mode(0o600)).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = lapis_content_mcp::router(grants.clone(), &base).unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            root,
            vault,
            grants,
            grant: issued.grant_id,
            token,
            base,
            server,
            _runtime: runtime,
        }
    }
    pub fn command(&self) -> Command {
        command(&self.base, &self.root.path().join("token"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
        self.grants.shutdown().unwrap();
    }
}
pub fn command(base: &str, token: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_lapis"));
    cmd.args(["--target", base, "--token-file"])
        .arg(token)
        .args(["--timeout-ms", "1000", "mcp", "serve", "--stdio"])
        .env("HTTP_PROXY", "http://127.0.0.1:1")
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    cmd
}
pub async fn finish(child: Child) -> std::process::Output {
    tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
        .await
        .unwrap()
        .unwrap()
}
