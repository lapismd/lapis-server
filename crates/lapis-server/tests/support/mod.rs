use lapis_app_protocol::{ClientDescriptor, PROTOCOL_MAJOR, Reply, Request, VaultCommand};
use lapis_server::{
    config::{Config, VaultConfig},
    host::Host,
    http::{VERSION_HEADER, router},
};
use reqwest::{Client, RequestBuilder};
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

pub const TOKEN: &str = "isolated-tests-owner-token-0123456789";
pub const ORIGIN: &str = "http://127.0.0.1:29999";

pub struct Fixture {
    pub roots: [TempDir; 2],
    pub ids: [Uuid; 2],
    pub host: Arc<Host>,
    pub base: String,
    pub client: Client,
    pub profile: Option<TempDir>,
    server: tokio::task::JoinHandle<()>,
}
impl Fixture {
    pub async fn start() -> Self {
        Self::start_with_database(false).await
    }
    pub async fn with_database() -> Self {
        Self::start_with_database(true).await
    }
    async fn start_with_database(selected: bool) -> Self {
        Self::start_with_services(selected, false).await
    }
    pub(crate) async fn start_with_services(selected: bool, indexed_history: bool) -> Self {
        let roots = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
        let ids = [Uuid::new_v4(), Uuid::new_v4()];
        let config = Config {
            vaults: (0..2)
                .map(|i| VaultConfig {
                    id: ids[i],
                    name: format!("Notes {i}"),
                    root: roots[i].path().into(),
                })
                .collect(),
        };
        let profile = selected.then(|| tempfile::tempdir().unwrap());
        let host = Arc::new(match &profile {
            Some(profile) if indexed_history => {
                Host::with_indexed_history(config, profile.path(), Vec::new()).unwrap()
            }
            Some(profile) => Host::with_profile(config, profile.path()).unwrap(),
            None => Host::new(config).unwrap(),
        });
        let app = router(
            host.clone(),
            lapis_server::http::Authentication::token(TOKEN).unwrap(),
            ORIGIN,
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            roots,
            ids,
            host,
            base,
            client: Client::new(),
            profile,
            server,
        }
    }
    pub fn authorize(&self, builder: RequestBuilder) -> RequestBuilder {
        builder
            .bearer_auth(TOKEN)
            .header(VERSION_HEADER, PROTOCOL_MAJOR)
    }
    pub async fn attach(&self, id: Uuid) -> ClientDescriptor {
        self.authorize(
            self.client
                .post(format!("{}/api/v3/vaults/{id}/clients", self.base)),
        )
        .json(&lapis_app_protocol::AttachClient {
            protocol_version: PROTOCOL_MAJOR,
        })
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
        self.authorize(self.client.post(format!(
            "{}/api/v3/clients/{}/commands",
            self.base, client.client_id
        )))
        .json(&Request {
            protocol_version: PROTOCOL_MAJOR,
            request_id: Uuid::new_v4(),
            vault_id: client.vault_id,
            generation: client.generation,
            command,
        })
    }
    pub async fn execute(&self, client: &ClientDescriptor, command: VaultCommand) -> Reply {
        self.command(client, command)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    pub async fn detach(&self, client: &ClientDescriptor) {
        let response = self
            .authorize(
                self.client
                    .delete(format!("{}/api/v3/clients/{}", self.base, client.client_id)),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 204);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
