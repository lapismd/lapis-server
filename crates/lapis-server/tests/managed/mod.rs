mod lifecycle;
mod recovery;
mod security;
mod transfers;

use lapis_app_protocol::{ClientDescriptor, ManagedVault, Reply, Request, VaultCommand};
use lapis_server::{
    config::{Config, VaultConfig},
    control::Control,
    host::Host,
    http::{Authentication, router},
};
use reqwest::RequestBuilder;
use serde_json::json;
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

pub const TOKEN: &str = "managed-vault-test-owner-0123456789abcdef";
pub const ORIGIN: &str = "https://notes.example";
pub struct Fixture {
    root: Option<tempfile::TempDir>,
    pub profile: PathBuf,
    pub configured: Uuid,
    pub host: Arc<Host>,
    control: Arc<Control>,
    pub base: String,
    server: Option<tokio::task::JoinHandle<()>>,
}
impl Fixture {
    pub async fn start() -> Self {
        Self::open(tempfile::tempdir().unwrap()).await
    }
    async fn open(root: tempfile::TempDir) -> Self {
        let vault = root.path().join("configured");
        let profile = root.path().join("profile");
        std::fs::create_dir_all(&vault).unwrap();
        std::fs::create_dir_all(&profile).unwrap();
        let identity = root.path().join("configured-id");
        let configured = if identity.exists() {
            Uuid::parse_str(&std::fs::read_to_string(identity).unwrap()).unwrap()
        } else {
            let id = Uuid::new_v4();
            std::fs::write(identity, id.to_string()).unwrap();
            id
        };
        let control = Arc::new(Control::open(&profile).await.unwrap());
        control
            .initialize("owner", crate::credentials::HASH, crate::credentials::NOW)
            .await
            .unwrap();
        let host = Arc::new(
            Host::with_profile(
                Config {
                    vaults: vec![VaultConfig {
                        id: configured,
                        name: "Configured".into(),
                        root: vault,
                    }],
                },
                &profile,
            )
            .unwrap(),
        );
        host.enable_management(control.clone(), &profile)
            .await
            .unwrap();
        let app = router(
            host.clone(),
            Authentication::browser(control.clone(), Some(TOKEN), false).unwrap(),
            ORIGIN,
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            root: Some(root),
            profile,
            configured,
            host,
            control,
            base,
            server: Some(server),
        }
    }
    pub fn post(&self, path: &str) -> RequestBuilder {
        reqwest::Client::new()
            .post(format!("{}{path}", self.base))
            .bearer_auth(TOKEN)
            .header("x-lapis-protocol", 3)
    }
    pub async fn list(&self) -> Vec<ManagedVault> {
        reqwest::Client::new()
            .get(format!("{}/api/v3/vaults", self.base))
            .bearer_auth(TOKEN)
            .header("x-lapis-protocol", 3)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    pub async fn create(&self, name: &str) -> ManagedVault {
        self.post("/api/v3/vaults")
            .json(&json!({"name": name}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    pub async fn attach(&self, id: Uuid) -> ClientDescriptor {
        self.post(&format!("/api/v3/vaults/{id}/clients"))
            .json(&json!({"protocol_version": 3}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    pub async fn execute(&self, client: &ClientDescriptor, command: VaultCommand) -> Reply {
        self.command(client, command)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    pub fn command(&self, client: &ClientDescriptor, command: VaultCommand) -> RequestBuilder {
        self.post(&format!("/api/v3/clients/{}/commands", client.client_id))
            .json(&Request {
                protocol_version: 3,
                request_id: Uuid::new_v4(),
                vault_id: client.vault_id,
                generation: client.generation,
                command,
            })
    }
    pub async fn action(&self, id: Uuid, action: &str) -> ManagedVault {
        self.post(&format!("/api/v3/vaults/{id}/{action}"))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
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
        self.control.close().await.unwrap();
    }
    pub async fn restart(mut self) -> Self {
        self.finish().await;
        let root = self.root.take().unwrap();
        drop(self);
        Self::open(root).await
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(server) = &self.server {
            server.abort();
        }
    }
}
