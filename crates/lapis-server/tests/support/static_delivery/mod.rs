use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    time::timeout,
};

pub const TOKEN: &str = "static-delivery-isolated-owner-token-0123456789";

pub struct Fixture {
    pub root: tempfile::TempDir,
    pub vault: PathBuf,
    pub assets: PathBuf,
    pub config: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let vault = root.path().join("vault");
        let assets = root.path().join("static");
        std::fs::create_dir(&vault).unwrap();
        std::fs::create_dir(&assets).unwrap();
        std::fs::write(
            assets.join("index.html"),
            "<!doctype html><title>Lapis fixture</title>",
        )
        .unwrap();
        std::fs::write(assets.join("app.css"), "body { color: black }").unwrap();
        std::fs::write(assets.join("app.12345678.css"), "body { color: black }").unwrap();
        std::fs::write(assets.join("app.wasm"), [0, 97, 115, 109]).unwrap();
        let config = root.path().join("server.json");
        std::fs::write(
            &config,
            serde_json::to_vec(&serde_json::json!({
                "vaults": [{ "id": uuid::Uuid::new_v4(), "name": "Static fixture", "root": vault }]
            }))
            .unwrap(),
        )
        .unwrap();
        Self {
            root,
            vault,
            assets,
            config,
        }
    }
    pub fn command(&self, assets: &std::path::Path) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lapis-server"));
        command
            .arg(&self.config)
            .arg("127.0.0.1:0")
            .env("LAPIS_SERVER_TOKEN", TOKEN)
            .env("LAPIS_SERVER_STATIC_ROOT", assets)
            .env_remove("LAPIS_SERVER_DATA_ROOT")
            .env_remove("LAPIS_SERVER_ORIGIN")
            .env_remove("LAPIS_SERVER_AGENTS_CONFIG")
            .env_remove("LAPIS_SERVER_BOOTSTRAP_USERNAME")
            .env_remove("LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        command
    }
    pub async fn start(&self) -> (Child, String) {
        let mut child = self.command(&self.assets).spawn().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
        let line = timeout(Duration::from_secs(10), output.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let address = line
            .strip_prefix("Lapis file service listening on ")
            .unwrap();
        (child, format!("http://{address}"))
    }
}
