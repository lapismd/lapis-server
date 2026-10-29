use lapis_app_protocol::{
    ClientDescriptor,
    browser::SessionInfo,
    turns::{TurnInput, TurnRecord},
};
use lapis_server::{
    config::{Config, VaultConfig},
    control::Control,
    host::{Host, turns::AgentConfig},
    http::{Authentication, VERSION_HEADER},
};
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, sync::Arc, time::Duration};
use uuid::Uuid;
pub const TOKEN: &str = "native-queue-http-fixture-token-0123456789";
pub const ORIGIN: &str = "http://localhost:29999";
pub struct Fixture {
    pub temp: tempfile::TempDir,
    pub profile: PathBuf,
    pub ids: [Uuid; 2],
    pub host: Arc<Host>,
    pub client: reqwest::Client,
    pub base: String,
    pub cookie: Option<(String, SessionInfo)>,
    control: Option<Arc<Control>>,
    server: tokio::task::JoinHandle<()>,
    selected: bool,
}
impl Fixture {
    pub async fn start(browser: bool, selected: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let profile = base.join("profile");
        fs::create_dir(&profile).unwrap();
        for i in 0..2 {
            fs::create_dir(base.join(format!("vault-{i}"))).unwrap();
        }
        let script = base.join("peer.py");
        fs::write(
            &script,
            include_str!("../../../../lapis-app-core/tests/support/native_queue/peer.py"),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        let ids = [Uuid::new_v4(), Uuid::new_v4()];
        let host = Self::host(&base, &profile, ids, selected);
        let control = if browser {
            let c = Arc::new(Control::open(&profile).await.unwrap());
            c.initialize("Owner", crate::credentials::HASH, crate::credentials::NOW)
                .await
                .unwrap();
            Some(c)
        } else {
            None
        };
        let (base, server) = Self::serve(host.clone(), control.clone()).await;
        let mut f = Self {
            temp,
            profile,
            ids,
            host,
            client: reqwest::Client::new(),
            base,
            cookie: None,
            control,
            server,
            selected,
        };
        if browser {
            f.cookie = Some(f.login().await);
        }
        f
    }
    fn host(
        base: &std::path::Path,
        profile: &std::path::Path,
        ids: [Uuid; 2],
        selected: bool,
    ) -> Arc<Host> {
        Arc::new(
            Host::with_profile_and_agents(
                Config {
                    vaults: (0..2)
                        .map(|i| VaultConfig {
                            id: ids[i],
                            name: format!("Queue {i}"),
                            root: base.join(format!("vault-{i}")),
                        })
                        .collect(),
                },
                profile,
                if selected {
                    vec![AgentConfig {
                        participant_selections: Vec::new(),
                        queue_selection: Some(serde_json::from_value(json!({
                            "recipient":{"id":"writer","label":"Writer","token":"jj-store-agent:777269746572;"},
                            "target":{"id":"writer","agent":"local","thinking":"default"}
                        })).unwrap()),
                        id: "fixture".into(),
                        label: "Operator fixture".into(),
                        default_mode: "plan".into(),
                        executable: base.join("peer.py"),
                        environment: std::collections::BTreeMap::from([
                            (
                                "LOG".into(),
                                base.join("commands.jsonl").to_string_lossy().into(),
                            ),
                            (
                                "RELEASE".into(),
                                base.join("release").to_string_lossy().into(),
                            ),
                            (
                                "PRIVATE_NATIVE_TOKEN".into(),
                                "not-in-wire-responses".into(),
                            ),
                        ]),
                    }]
                } else {
                    vec![]
                },
            )
            .unwrap(),
        )
    }
    async fn serve(
        host: Arc<Host>,
        control: Option<Arc<Control>>,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let auth = match control {
            Some(c) => Authentication::browser(c, Some(TOKEN), true).unwrap(),
            None => Authentication::token(TOKEN).unwrap(),
        };
        let app = lapis_server::http::router(host, auth, ORIGIN).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        (
            base,
            tokio::spawn(async move {
                axum::serve(
                    listener,
                    app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
                )
                .await
                .unwrap();
            }),
        )
    }
    pub fn raw(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.post(format!("{}{path}", self.base))
    }
    pub fn auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let request = request.header(VERSION_HEADER, 3);
        match &self.cookie {
            Some((cookie, info)) => request
                .header("cookie", cookie)
                .header("origin", ORIGIN)
                .header("x-lapis-csrf", &info.csrf_token),
            None => request.bearer_auth(TOKEN),
        }
    }
    pub async fn login(&self) -> (String, SessionInfo) {
        let reply = self
            .raw("/api/v3/auth/login")
            .header(VERSION_HEADER, 3)
            .header("origin", ORIGIN)
            .json(&json!({"username":"owner","password":crate::credentials::PASSWORD}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        let cookie = reply.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        (cookie, reply.json().await.unwrap())
    }
    pub async fn attach(&self, index: usize) -> ClientDescriptor {
        self.auth(self.raw(&format!("/api/v3/vaults/{}/clients", self.ids[index])))
            .json(&json!({"protocol_version":3}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    pub fn queue(&self, client: Uuid, value: Value) -> reqwest::RequestBuilder {
        self.auth(self.raw(&format!("/api/v3/clients/{client}/queues")))
            .json(&value)
    }
    pub async fn call(&self, client: Uuid, value: Value) -> Value {
        let reply = self.queue(client, value).send().await.unwrap();
        let status = reply.status();
        let value: Value = reply.json().await.unwrap();
        assert!(status.is_success(), "{status}: {value}");
        value["queue"].clone()
    }
    pub async fn first(&self, client: Uuid) -> TurnRecord {
        let input = TurnInput {
            request_id: Uuid::new_v4(),
            target_id: "fixture".into(),
            scope_dir: "".into(),
            prompt: "held-first".into(),
            context: None,
            continuation: None,
            options: Default::default(),
        };
        let reply: Value = self
            .auth(self.raw(&format!("/api/v3/clients/{client}/turns")))
            .json(&json!({"kind":"submit","input":input}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        self.until(|| self.prompts() == 1).await;
        serde_json::from_value(reply["record"].clone()).unwrap()
    }
    pub fn accept(&self, head: &TurnRecord, name: &str) -> Value {
        json!({"kind":"accept","input":{"acceptance_id":Uuid::new_v4(),"conversation_id":head.conversation_id,"after_request_id":head.input.request_id,"scope_dir":"","target_id":"fixture","capture":{"version":1,"id":format!("capture-{name}"),"state":"queued","user":{"schemaVersion":3,"id":format!("queued-user-{name}"),"type":"message","role":"user","text":format!("Authored {name} café 📝"),"createdAt":"2026-10-27T00:00:00.000Z","recipients":[{"id":"writer","label":"Writer","token":"jj-store-agent:777269746572;"}]},"selection":{"runtime":"acp","thinking":"high","targets":[{"id":"writer","agent":"codex","thinking":"default"}],"attachments":[]}}}})
    }
    pub fn release(&self) {
        fs::write(self.temp.path().join("release"), b"ready").unwrap();
    }
    pub fn prompts(&self) -> usize {
        self.commands()
            .iter()
            .filter(|v| v["type"] == "prompt")
            .count()
    }
    pub fn commands(&self) -> Vec<Value> {
        fs::read_to_string(self.temp.path().join("commands.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|s| serde_json::from_str(s).ok())
            .collect()
    }
    pub async fn until(&self, ready: impl Fn() -> bool) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while !ready() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
    pub fn stored(&self, conversation: Uuid) -> Option<Value> {
        fs::read(
            self.profile
                .join("turns/queues")
                .join(self.ids[0].to_string())
                .join(format!("{conversation}.json")),
        )
        .ok()
        .and_then(|s| serde_json::from_slice(&s).ok())
    }
    pub async fn detach(&self, client: Uuid) {
        self.auth(
            self.client
                .delete(format!("{}/api/v3/clients/{client}", self.base)),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    }
    pub async fn restart(&mut self) {
        self.close().await;
        self.host = Self::host(
            &self.temp.path().canonicalize().unwrap(),
            &self.profile,
            self.ids,
            self.selected,
        );
        let control = if self.control.is_some() {
            Some(Arc::new(Control::open(&self.profile).await.unwrap()))
        } else {
            None
        };
        let (base, server) = Self::serve(self.host.clone(), control.clone()).await;
        self.control = control;
        self.base = base;
        self.server = server;
    }
    pub async fn close(&self) {
        let host = self.host.clone();
        tokio::task::spawn_blocking(move || host.shutdown())
            .await
            .unwrap()
            .unwrap();
        self.server.abort();
        if let Some(c) = &self.control {
            c.close().await.unwrap();
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
