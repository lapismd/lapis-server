use crate::credentials::{HASH, NOW, PASSWORD};
use lapis_app_protocol::{
    ClientDescriptor, PROTOCOL_MAJOR, Request, VaultCommand, browser::SessionInfo,
};
use lapis_server::{
    config::{Config, VaultConfig},
    control::Control,
    host::Host,
    http::{Authentication, VERSION_HEADER, router},
};
use reqwest::RequestBuilder;
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

pub struct BrowserFixture {
    _root: tempfile::TempDir,
    pub vault: PathBuf,
    pub id: Uuid,
    pub host: Arc<Host>,
    pub control: Arc<Control>,
    pub client: reqwest::Client,
    pub base: String,
    pub origin: &'static str,
    server: tokio::task::JoinHandle<()>,
}
impl BrowserFixture {
    pub async fn start(insecure: bool, token: Option<&str>) -> Self {
        let root = tempfile::tempdir().unwrap();
        let vault = root.path().join("vault");
        std::fs::create_dir(&vault).unwrap();
        let profile = root.path().join("profile");
        std::fs::create_dir(&profile).unwrap();
        let id = Uuid::new_v4();
        let host = Arc::new(
            Host::with_profile(
                Config {
                    vaults: vec![VaultConfig {
                        id,
                        name: "Private Notes".into(),
                        root: vault.clone(),
                    }],
                },
                &profile,
            )
            .unwrap(),
        );
        let control = Arc::new(Control::open(&profile).await.unwrap());
        control.initialize("Owner", HASH, NOW).await.unwrap();
        let origin = if insecure {
            "http://localhost:29999"
        } else {
            "https://notes.example"
        };
        let app = router(
            host.clone(),
            Authentication::browser(control.clone(), token, insecure).unwrap(),
            origin,
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
            .unwrap();
        });
        Self {
            _root: root,
            vault,
            id,
            host,
            control,
            client: reqwest::Client::new(),
            base,
            origin,
            server,
        }
    }
    pub fn authorize(
        &self,
        builder: RequestBuilder,
        cookie: &str,
        info: &SessionInfo,
        mutate: bool,
    ) -> RequestBuilder {
        let builder = builder
            .header("cookie", cookie)
            .header(VERSION_HEADER, PROTOCOL_MAJOR);
        if mutate {
            builder
                .header("origin", self.origin)
                .header("x-lapis-csrf", &info.csrf_token)
        } else {
            builder
        }
    }
    pub async fn login(&self) -> (String, SessionInfo, String) {
        let response = self
            .client
            .post(format!("{}/api/v3/auth/login", self.base))
            .header("origin", self.origin)
            .header(VERSION_HEADER, PROTOCOL_MAJOR)
            .json(&serde_json::json!({"username":"owner","password":PASSWORD}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let header = response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .to_owned();
        let cookie = header.split(';').next().unwrap().to_owned();
        let info = response.json().await.unwrap();
        (cookie, info, header)
    }
    pub async fn attach(&self, cookie: &str, info: &SessionInfo) -> ClientDescriptor {
        self.authorize(
            self.client
                .post(format!("{}/api/v3/vaults/{}/clients", self.base, self.id)),
            cookie,
            info,
            true,
        )
        .json(&serde_json::json!({"protocol_version":PROTOCOL_MAJOR}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
    }
    pub fn command(
        &self,
        cookie: &str,
        info: &SessionInfo,
        attached: &ClientDescriptor,
        command: VaultCommand,
    ) -> RequestBuilder {
        self.authorize(
            self.client.post(format!(
                "{}/api/v3/clients/{}/commands",
                self.base, attached.client_id
            )),
            cookie,
            info,
            true,
        )
        .json(&Request {
            protocol_version: PROTOCOL_MAJOR,
            request_id: Uuid::new_v4(),
            vault_id: attached.vault_id,
            generation: attached.generation,
            command,
        })
    }
    pub async fn finish(&mut self) {
        self.server.abort();
        let _ = (&mut self.server).await;
        let host = self.host.clone();
        tokio::task::spawn_blocking(move || host.shutdown())
            .await
            .unwrap()
            .unwrap();
        self.control.close().await.unwrap();
    }
}
impl Drop for BrowserFixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
