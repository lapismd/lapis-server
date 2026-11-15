use lapis_app_protocol::{ClientDescriptor, PROTOCOL_MAJOR};
use lapis_server::{
    config::{Config, VaultConfig},
    host::{Host, RemoteInstallation},
    http::{Authentication, VERSION_HEADER, router},
};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};
use uuid::Uuid;
mod transport;

const TOKEN: &str = "disposable-remotes-owner-token-0123456789";
const SECRET: &str = "disposable-https-credential-fixture";

struct Running {
    host: Arc<Host>,
    task: tokio::task::JoinHandle<()>,
    client: reqwest::Client,
    base: String,
}
impl Running {
    async fn start(root: &Path, key: Option<&Path>, ids: [Uuid; 2]) -> Self {
        Self::start_with_trust(root, key, ids, None).await
    }
    async fn start_with_trust(
        root: &Path,
        key: Option<&Path>,
        ids: [Uuid; 2],
        trust: Option<&Path>,
    ) -> Self {
        let profile = root.join("profile");
        std::fs::create_dir_all(&profile).unwrap();
        let vaults = ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let path = root.join(format!("vault-{i}"));
                std::fs::create_dir_all(&path).unwrap();
                VaultConfig {
                    id: *id,
                    name: format!("Vault {i}"),
                    root: path,
                }
            })
            .collect();
        let host = Arc::new(
            Host::with_remote_profile(
                Config { vaults },
                &profile,
                vec![],
                false,
                RemoteInstallation {
                    key_file: key.map(Into::into),
                    known_hosts: trust.map(Into::into),
                    ssh_available: trust.is_some(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
        let app = router(
            host.clone(),
            Authentication::token(TOKEN).unwrap(),
            "https://notes.example",
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            host,
            task,
            client: reqwest::Client::new(),
            base,
        }
    }
    fn auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request
            .bearer_auth(TOKEN)
            .header(VERSION_HEADER, PROTOCOL_MAJOR)
    }
    async fn attach(&self, id: Uuid) -> ClientDescriptor {
        self.auth(
            self.client
                .post(format!("{}/api/v3/vaults/{id}/clients", self.base)),
        )
        .json(&json!({"protocol_version":PROTOCOL_MAJOR}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
    }
    fn url(&self, c: &ClientDescriptor) -> String {
        format!(
            "{}/api/v3/clients/{}/repository-remotes?vault={}&generation={}",
            self.base, c.client_id, c.vault_id, c.generation
        )
    }
    async fn settings(&self, c: &ClientDescriptor) -> Value {
        self.auth(self.client.get(self.url(c)))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    async fn control(&self, c: &ClientDescriptor, command: Value) -> reqwest::Response {
        self.auth(self.client.post(self.url(c)))
            .json(&command)
            .send()
            .await
            .unwrap()
    }
    async fn stop(self) {
        self.task.abort();
        let _ = self.task.await;
        let host = self.host;
        tokio::task::spawn_blocking(move || {
            host.shutdown().unwrap();
        })
        .await
        .unwrap();
    }
}
fn credential(revision: &Value, replace: Option<&str>, token: &str) -> Value {
    json!({"kind":"save_credential", "expectedRevision":revision,
        "input":{"label":"Hosted token", "provider":"https", "host":"git.example", "username":"owner", "token":token, "replace":replace}})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn encrypted_startup_routes_preserve_identity_and_credentials_across_restart() {
    let root = tempfile::tempdir().unwrap();
    let key = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(key.path(), [19; 32]).unwrap();
    let ids = [Uuid::new_v4(), Uuid::new_v4()];
    let f = Running::start(root.path(), Some(key.path()), ids).await;
    let c = f.attach(ids[0]).await;
    let initial = f.settings(&c).await;
    assert_eq!(initial["settings"]["host"], "server");
    assert_eq!(initial["settings"]["storage"], "available");
    assert_eq!(initial["settings"]["automaticSync"], false);
    let saved: Value = f
        .control(&c, credential(&initial["revision"], None, SECRET))
        .await
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!saved.to_string().contains(SECRET));
    let id = saved["settings"]["credentials"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let connection = json!({"name":"origin", "url":"https://git.example/owner/backup.git", "branch":"main", "bookmark":"", "authentication":"token", "credential":id});
    let configured: Value = f.control(&c, json!({"kind":"save_connection", "expectedRevision":saved["revision"], "target":"backup", "connection":connection})).await
        .error_for_status().unwrap().json().await.unwrap();
    assert_eq!(configured["settings"]["automaticSync"], false);
    assert!(configured["settings"]["targets"][0]["observation"].is_null());
    let other = f.attach(ids[1]).await;
    assert!(f.settings(&other).await["settings"]["targets"][0]["connection"].is_null());
    assert_eq!(
        f.control(&c, credential(&initial["revision"], None, "stale-fixture"))
            .await
            .status(),
        409
    );
    let replaced: Value = f
        .control(
            &c,
            credential(&configured["revision"], Some(&id), "replacement-fixture"),
        )
        .await
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(replaced["settings"]["credentials"][0]["id"], id);
    f.stop().await;
    let identity =
        std::fs::read(root.path().join("profile/repository-remotes/identity.json")).unwrap();
    let wrong = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(wrong.path(), [27; 32]).unwrap();
    for key_path in [None, Some(wrong.path())] {
        let f = Running::start(root.path(), key_path, ids).await;
        let c = f.attach(ids[0]).await;
        let locked = f.settings(&c).await;
        assert_eq!(locked["settings"]["storage"], "locked");
        assert!(
            locked["settings"]["credentials"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            f.control(&c, credential(&locked["revision"], None, SECRET))
                .await
                .status(),
            403
        );
        f.stop().await;
    }
    let f = Running::start(root.path(), Some(key.path()), ids).await;
    let c = f.attach(ids[0]).await;
    let restored = f.settings(&c).await;
    assert_eq!(restored["settings"]["credentials"][0]["id"], id);
    assert_eq!(
        restored["settings"]["targets"][0]["connection"],
        json!({"name":"origin", "url":"https://git.example/owner/backup.git", "branch":"main", "bookmark":"lapis-backup", "authentication":"token", "credential":id})
    );
    let forgotten: Value = f
        .control(
            &c,
            json!({"kind":"forget_credential", "expectedRevision":restored["revision"], "id":id}),
        )
        .await
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        forgotten["settings"]["credentials"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        forgotten["settings"]["targets"][0]["status"],
        "credentials-required"
    );
    assert_eq!(
        std::fs::read(root.path().join("profile/repository-remotes/identity.json")).unwrap(),
        identity
    );
    f.stop().await;
    fn inspect(path: &Path) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                inspect(&path);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                for secret in [SECRET, "replacement-fixture"] {
                    assert!(!bytes.windows(secret.len()).any(|v| v == secret.as_bytes()));
                }
            }
        }
    }
    inspect(root.path());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_http_admission_refuses_foreign_scope_paths_oversize_and_retired_clients() {
    let root = tempfile::tempdir().unwrap();
    let ids = [Uuid::new_v4(), Uuid::new_v4()];
    let f = Running::start(root.path(), None, ids).await;
    let c = f.attach(ids[0]).await;
    let url = f.url(&c);
    assert_eq!(f.client.get(&url).send().await.unwrap().status(), 401);
    assert_eq!(
        f.client
            .get(&url)
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        f.auth(f.client.get(&url))
            .header("origin", "https://foreign.example")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let foreign = url.replace(&c.generation.to_string(), &Uuid::new_v4().to_string());
    assert_eq!(
        f.auth(f.client.get(foreign)).send().await.unwrap().status(),
        403
    );
    let foreign = url.replace(&c.vault_id.to_string(), &ids[1].to_string());
    assert_eq!(
        f.auth(f.client.get(foreign)).send().await.unwrap().status(),
        403
    );
    let foreign = f
        .host
        .attach(
            ids[1],
            lapis_server::host::Principal::Session(Uuid::new_v4()),
        )
        .unwrap();
    assert_eq!(
        f.auth(f.client.get(f.url(&foreign)))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.control(
            &c,
            json!({"kind":"fetch", "target":"backup", "expectedRevision":0, "root":"/outside"})
        )
        .await
        .status(),
        422
    );
    assert_eq!(f.control(&c, json!({"kind":"save_credential", "expectedRevision":0, "input":{"provider":"https", "host":"git.example", "label":"x", "username":"owner", "token":"x".repeat(70_000)}})).await.status(), 413);
    let working = url.replace("repository-remotes", "repository-working-copy");
    let response = f.auth(f.client.get(working)).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let frame: Value = response.json().await.unwrap();
    assert_eq!(frame["source"], "backup");
    f.auth(
        f.client
            .delete(format!("{}/api/v3/clients/{}", f.base, c.client_id)),
    )
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap();
    assert_eq!(
        f.auth(f.client.get(url)).send().await.unwrap().status(),
        403
    );
    f.stop().await;
}
