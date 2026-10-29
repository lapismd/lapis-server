use lapis_app_protocol::{
    ClientDescriptor,
    turns::{TurnContinuation, TurnInput, TurnPhase, TurnRecord},
};
use lapis_server::{
    config::{Config, VaultConfig},
    host::{Host, turns::AgentConfig},
};
use reqwest::StatusCode;
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, sync::Arc, time::Duration};
use uuid::Uuid;

const TOKEN: &str = "native-turn-owner-fixture-token-0123456789";
const ORIGIN: &str = "http://127.0.0.1:29999";

struct Native {
    selected: bool,
    temp: tempfile::TempDir,
    ids: [Uuid; 2],
    profile: PathBuf,
    helper: PathBuf,
    log: PathBuf,
    host: Arc<Host>,
    base: String,
    client: reqwest::Client,
    server: tokio::task::JoinHandle<()>,
}
impl Native {
    async fn start() -> Self {
        Self::selected(true).await
    }
    async fn selected(selected: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let profile = base.join("profile");
        let helper = base.join("peer.py");
        let log = base.join("worker.jsonl");
        fs::create_dir(&profile).unwrap();
        for i in 0..2 {
            fs::create_dir(base.join(format!("vault-{i}"))).unwrap();
        }
        fs::write(
            &helper,
            include_str!("../../lapis-app-core/tests/support/native_turn/peer.py"),
        )
        .unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
        let ids = [Uuid::new_v4(), Uuid::new_v4()];
        let host = Self::host(&base, &profile, &helper, &log, ids, selected);
        let (url, server) = Self::serve(host.clone()).await;
        Self {
            selected,
            temp,
            ids,
            profile,
            helper,
            log,
            host,
            base: url,
            client: reqwest::Client::new(),
            server,
        }
    }
    fn host(
        base: &std::path::Path,
        profile: &std::path::Path,
        helper: &std::path::Path,
        log: &std::path::Path,
        ids: [Uuid; 2],
        selected: bool,
    ) -> Arc<Host> {
        Arc::new(
            Host::with_profile_and_agents(
                Config {
                    vaults: (0..2)
                        .map(|i| VaultConfig {
                            id: ids[i],
                            name: format!("Vault {i}"),
                            root: base.join(format!("vault-{i}")),
                        })
                        .collect(),
                },
                profile,
                if selected {
                    vec![AgentConfig {
                        participant_selections: Vec::new(),
                        queue_selection: None,
                        id: "fixture".into(),
                        label: "Fixture agent".into(),
                        default_mode: "plan".into(),
                        executable: helper.into(),
                        environment: std::collections::BTreeMap::from([
                            ("LOG".into(), log.to_string_lossy().into()),
                            (
                                "PRIVATE_NATIVE_TOKEN".into(),
                                "not-in-browser-responses".into(),
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
    async fn serve(host: Arc<Host>) -> (String, tokio::task::JoinHandle<()>) {
        let app = lapis_server::http::router(
            host,
            lapis_server::http::Authentication::token(TOKEN).unwrap(),
            ORIGIN,
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        (
            base,
            tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            }),
        )
    }
    fn request(&self, path: &str) -> reqwest::RequestBuilder {
        self.client
            .post(format!("{}{path}", self.base))
            .bearer_auth(TOKEN)
            .header("x-lapis-protocol", lapis_app_protocol::PROTOCOL_MAJOR)
    }
    async fn attach(&self, index: usize) -> ClientDescriptor {
        self.request(&format!("/api/v3/vaults/{}/clients", self.ids[index]))
            .json(&json!({"protocol_version":lapis_app_protocol::PROTOCOL_MAJOR}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    fn command(&self, client: Uuid, value: serde_json::Value) -> reqwest::RequestBuilder {
        self.request(&format!("/api/v3/clients/{client}/turns"))
            .json(&value)
    }
    async fn call(&self, client: Uuid, value: serde_json::Value) -> serde_json::Value {
        self.command(client, value)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    fn input(&self) -> TurnInput {
        TurnInput {
            request_id: Uuid::new_v4(),
            continuation: None,
            target_id: "fixture".into(),
            scope_dir: "".into(),
            prompt: "Authored café 📝".into(),
            context: Some("Separate context".into()),
            options: Default::default(),
        }
    }
    async fn terminal(&self, id: Uuid) -> TurnRecord {
        let path = self
            .profile
            .join("turns")
            .join(self.ids[0].to_string())
            .join(format!("{id}.json"));
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                if let Ok(bytes) = fs::read(&path) {
                    let record: TurnRecord = serde_json::from_slice(&bytes).unwrap();
                    if !matches!(record.phase, TurnPhase::Accepted | TurnPhase::Running) {
                        return record;
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
    fn prompts(&self) -> usize {
        fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .filter(|line| {
                serde_json::from_str::<serde_json::Value>(line).unwrap()["type"] == "prompt"
            })
            .count()
    }
    async fn close(&self) {
        let host = self.host.clone();
        tokio::task::spawn_blocking(move || host.shutdown())
            .await
            .unwrap()
            .unwrap();
    }
    async fn restart(&mut self) {
        self.close().await;
        self.server.abort();
        self.host = Self::host(
            &self.temp.path().canonicalize().unwrap(),
            &self.profile,
            &self.helper,
            &self.log,
            self.ids,
            self.selected,
        );
        let (base, server) = Self::serve(self.host.clone()).await;
        self.base = base;
        self.server = server;
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        self.server.abort();
    }
}

#[tokio::test]
async fn admitted_turn_commands_report_unselected_service_without_fallback() {
    let f = Native::selected(false).await;
    let client = f.attach(0).await;
    let response = f
        .command(client.client_id, json!({"kind":"list"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["code"],
        "UNSUPPORTED"
    );
    let response = f
        .client
        .post(format!(
            "{}/api/v3/clients/{}/turns",
            f.base, client.client_id
        ))
        .json(&json!({"kind":"list"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hosted_turn_completes_without_client_and_reconnects_after_cold_owner_restart() {
    let mut f = Native::start().await;
    let client = f.attach(0).await;
    let input = f.input();
    let targets = f.call(client.client_id, json!({"kind":"targets"})).await;
    assert_eq!(
        targets,
        json!({"kind":"targets","targets":[{"id":"fixture","label":"Fixture agent","default_mode":"plan"}]})
    );
    let value = f
        .call(client.client_id, json!({"kind":"submit","input":input}))
        .await;
    let accepted: TurnRecord = serde_json::from_value(value["record"].clone()).unwrap();
    assert_eq!(accepted.input.options.mode, "ask");
    let response = f
        .client
        .delete(format!("{}/api/v3/clients/{}", f.base, client.client_id))
        .bearer_auth(TOKEN)
        .header("x-lapis-protocol", 3)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let completed = f.terminal(input.request_id).await;
    assert_eq!(completed.phase, TurnPhase::Completed);
    assert_eq!(accepted.conversation_id, completed.conversation_id);
    let record_path = f
        .temp
        .path()
        .join("vault-0/.lapis/agents/sessions")
        .join(completed.conversation_id.to_string())
        .join("transcript.jsonl");
    let text = fs::read_to_string(record_path).unwrap();
    assert!(text.contains("reply café 📝"));
    assert!(!text.contains("Separate context"));
    let replacement = f.attach(0).await;
    let read = f
        .call(
            replacement.client_id,
            json!({"kind":"read","request_id":input.request_id}),
        )
        .await;
    assert_eq!(read["record"], serde_json::to_value(&completed).unwrap());
    let duplicate = f
        .call(
            replacement.client_id,
            json!({"kind":"submit","input":input}),
        )
        .await;
    assert_eq!(duplicate, read);
    let other = f.attach(1).await;
    let foreign = f
        .call(
            other.client_id,
            json!({"kind":"read","request_id":input.request_id}),
        )
        .await;
    assert!(foreign["record"].is_null());
    let list = f.call(replacement.client_id, json!({"kind":"list"})).await;
    assert_eq!(list["records"].as_array().unwrap().len(), 1);
    assert!(!list.to_string().contains("prompt"));
    assert!(!list.to_string().contains("output"));
    assert!(!list.to_string().contains("not-in-browser-responses"));
    f.restart().await;
    let reopened = f.attach(0).await;
    let duplicate = f
        .call(reopened.client_id, json!({"kind":"submit","input":input}))
        .await;
    assert_eq!(duplicate, read);
    assert_eq!(f.prompts(), 1);
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hosted_turn_commands_keep_native_launch_authority_and_exact_cancel_scope() {
    let f = Native::start().await;
    let client = f.attach(0).await;
    for key in ["root", "executable", "environment", "arguments", "resume"] {
        let mut input = serde_json::to_value(f.input()).unwrap();
        input[key] = json!("foreign");
        let response = f
            .command(client.client_id, json!({"kind":"submit","input":input}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY, "{key}");
    }
    let response = f
        .command(client.client_id, json!({"kind":"list"}))
        .header("origin", "https://foreign.example")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let mut input = f.input();
    input.request_id = Uuid::nil();
    let response = f
        .command(client.client_id, json!({"kind":"submit","input":input}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let input = f.input();
    f.call(client.client_id, json!({"kind":"submit","input":input}))
        .await;
    let cancel = f
        .call(
            client.client_id,
            json!({"kind":"cancel","request_id":input.request_id}),
        )
        .await;
    assert_eq!(cancel, json!({"kind":"cancel_requested","accepted":true}));
    let record = f.terminal(input.request_id).await;
    assert!(matches!(
        record.phase,
        TurnPhase::Cancelled | TurnPhase::Unknown
    ));
    let duplicate = f
        .call(client.client_id, json!({"kind":"submit","input":input}))
        .await;
    assert_eq!(duplicate["record"], serde_json::to_value(record).unwrap());
    let terminal_cancel = f
        .call(
            client.client_id,
            json!({"kind":"cancel","request_id":input.request_id}),
        )
        .await;
    assert_eq!(
        terminal_cancel,
        json!({"kind":"cancel_requested","accepted":false})
    );
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hosted_disable_fences_retained_clients_without_affecting_another_vault() {
    let f = Native::start().await;
    let first = f.attach(0).await;
    let retained = f.attach(0).await;
    let other = f.attach(1).await;
    let input = f.input();
    f.call(first.client_id, json!({"kind":"submit","input":input}))
        .await;
    let disabled = f
        .call(
            first.client_id,
            json!({"kind":"set_enabled","enabled":false}),
        )
        .await;
    assert_eq!(disabled, json!({"kind":"availability","enabled":false}));
    let terminal = f.terminal(input.request_id).await;
    let duplicate = f
        .call(retained.client_id, json!({"kind":"submit","input":input}))
        .await;
    assert_eq!(duplicate["record"], serde_json::to_value(terminal).unwrap());
    let refused = f
        .command(
            retained.client_id,
            json!({"kind":"submit","input":f.input()}),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::NOT_IMPLEMENTED);
    assert_eq!(
        refused.json::<serde_json::Value>().await.unwrap()["code"],
        "UNSUPPORTED"
    );
    assert_eq!(
        f.call(other.client_id, json!({"kind":"targets"})).await["targets"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let prompts = f.prompts();
    assert_eq!(
        f.call(
            first.client_id,
            json!({"kind":"set_enabled","enabled":true})
        )
        .await,
        json!({"kind":"availability","enabled":true})
    );
    assert_eq!(f.prompts(), prompts);
    let fresh = f.input();
    f.call(retained.client_id, json!({"kind":"submit","input":fresh}))
        .await;
    assert_eq!(
        f.terminal(fresh.request_id).await.phase,
        TurnPhase::Completed
    );
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hosted_followups_keep_one_conversation_across_detach_and_cold_recovery() {
    let mut f = Native::start().await;
    let client = f.attach(0).await;
    let first = f.input();
    f.call(client.client_id, json!({"kind":"submit","input":first}))
        .await;
    let first = f.terminal(first.request_id).await;
    assert_eq!(first.phase, TurnPhase::Completed);
    let path = f
        .temp
        .path()
        .join("vault-0/.lapis/agents/sessions")
        .join(first.conversation_id.to_string())
        .join("transcript.jsonl");
    let original = fs::read_to_string(&path).unwrap();
    let mut followup = f.input();
    followup.prompt = "A hosted follow-up".into();
    followup.continuation = Some(TurnContinuation {
        conversation_id: first.conversation_id,
        after_request_id: first.input.request_id,
    });
    f.call(client.client_id, json!({"kind":"submit","input":followup}))
        .await;
    let response = f
        .client
        .delete(format!("{}/api/v3/clients/{}", f.base, client.client_id))
        .bearer_auth(TOKEN)
        .header("x-lapis-protocol", 3)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let second = f.terminal(followup.request_id).await;
    assert_eq!(second.phase, TurnPhase::Completed);
    assert_eq!(second.conversation_id, first.conversation_id);
    assert!(fs::read_to_string(&path).unwrap().starts_with(&original));
    f.restart().await;
    let client = f.attach(0).await;
    let duplicate = f
        .call(client.client_id, json!({"kind":"submit","input":followup}))
        .await;
    assert_eq!(duplicate["record"], serde_json::to_value(&second).unwrap());
    assert_eq!(f.prompts(), 2);
    let mut stale = followup.clone();
    stale.request_id = Uuid::new_v4();
    let response = f
        .command(client.client_id, json!({"kind":"submit","input":stale}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["code"],
        "BUSY"
    );
    followup.request_id = Uuid::new_v4();
    followup.continuation.as_mut().unwrap().after_request_id = second.input.request_id;
    f.call(client.client_id, json!({"kind":"submit","input":followup}))
        .await;
    let third = f.terminal(followup.request_id).await;
    assert_eq!(third.phase, TurnPhase::Completed);
    assert_eq!(third.conversation_id, first.conversation_id);
    assert_eq!(f.prompts(), 3);
    f.close().await;
}
