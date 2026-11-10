use lapis_app_protocol::{ClientDescriptor, PROTOCOL_MAJOR};
use lapis_server::{
    config::{Config, VaultConfig},
    host::Host,
    http::{Authentication, router},
};
use std::sync::Arc;
use uuid::Uuid;
const TOKEN: &str = "terminal-qualification-owner-token-0123456789";
struct Fixture {
    _roots: [tempfile::TempDir; 2],
    ids: [Uuid; 2],
    host: Arc<Host>,
    base: String,
    client: reqwest::Client,
    server: tokio::task::JoinHandle<()>,
}
impl Fixture {
    async fn start() -> Self {
        Self::with_auth(Authentication::token(TOKEN).unwrap()).await
    }
    async fn with_auth(authentication: Authentication) -> Self {
        let roots = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
        let ids = [Uuid::new_v4(), Uuid::new_v4()];
        let host = Arc::new(
            Host::new(Config {
                vaults: (0..2)
                    .map(|i| VaultConfig {
                        id: ids[i],
                        name: format!("Vault {i}"),
                        root: roots[i].path().into(),
                    })
                    .collect(),
            })
            .unwrap(),
        );
        let app = router(host.clone(), authentication, "http://localhost").unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            _roots: roots,
            ids,
            host,
            base,
            client: reqwest::Client::new(),
            server,
        }
    }
    fn authorize(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request
            .bearer_auth(TOKEN)
            .header("x-lapis-protocol", PROTOCOL_MAJOR)
    }
    async fn attach(&self, id: Uuid) -> ClientDescriptor {
        self.authorize(
            self.client
                .post(format!("{}/api/v3/vaults/{id}/clients", self.base)),
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
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
use reqwest::StatusCode;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn terminal_output_releases_the_connection_while_the_shell_is_running() {
    let f = Fixture::start().await;
    let owner = f.attach(f.ids[0]).await;
    let url = format!("{}/api/v3/clients/{}/terminal", f.base, owner.client_id);
    let created: serde_json::Value = f
        .authorize(f.client.post(&url))
        .json(&json!({"kind":"create","cwd":null,"cols":80,"rows":24}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let session = created["session_id"].as_str().unwrap();
    let response = f
        .authorize(f.client.get(format!("{url}/{session}/events")))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let body = tokio::time::timeout(std::time::Duration::from_secs(3), response.text())
        .await
        .expect("a live shell must not monopolize an HTTP/1 connection")
        .unwrap();
    assert!(body.contains("\"running\":true"));
    assert!(body.contains("event: output"));
    f.host.shutdown().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authenticated_terminal_routes_are_client_scoped_and_revoke_output() {
    let f = Fixture::start().await;
    let owner = f.attach(f.ids[0]).await;
    let foreign = f.attach(f.ids[1]).await;
    let url = format!("{}/api/v3/clients/{}/terminal", f.base, owner.client_id);
    assert_eq!(
        f.client
            .post(&url)
            .json(&json!({"kind":"close_all"}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let created: serde_json::Value = f
        .authorize(f.client.post(&url))
        .json(&json!({"kind":"create","cwd":null,"cols":80,"rows":24}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let session = created["session_id"].as_str().unwrap();
    assert_eq!(
        f.authorize(f.client.post(format!(
            "{}/api/v3/clients/{}/terminal",
            f.base, foreign.client_id
        )))
        .json(&json!({"kind":"write","session_id":session,"data":"echo forbidden\n"}))
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::FORBIDDEN
    );
    let stream = format!("{url}/{session}/events");
    let mut response = f
        .authorize(f.client.get(&stream))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream")
    );
    let first = tokio::time::timeout(std::time::Duration::from_secs(3), response.chunk())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&first).contains("event: output"));
    f.authorize(
        f.client
            .delete(format!("{}/api/v3/clients/{}", f.base, owner.client_id)),
    )
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap();
    assert_eq!(
        f.authorize(f.client.get(&stream))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.authorize(f.client.post(&url))
            .json(&json!({"kind":"create","cwd":null,"cols":80,"rows":24}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn terminal_principal_revocation_fences_the_owner_without_retiring_other_clients() {
    use lapis_app_protocol::terminal::{TerminalCommand as C, TerminalValue};
    use lapis_server::host::Principal;
    let f = Fixture::start().await;
    let owner = Principal::Session(uuid::Uuid::new_v4());
    let other = Principal::Session(uuid::Uuid::new_v4());
    let a = f.host.attach(f.ids[0], owner).unwrap();
    let b = f.host.attach(f.ids[0], other).unwrap();
    let value = f
        .host
        .terminal(
            a.client_id,
            owner,
            C::Create {
                cwd: None,
                cols: 80,
                rows: 24,
            },
        )
        .unwrap();
    let TerminalValue::Created { session_id } = value else {
        panic!("missing terminal")
    };
    assert!(
        f.host
            .terminal_output(a.client_id, other, session_id, 0)
            .is_err()
    );
    f.host.detach_principal(owner).unwrap();
    assert!(
        f.host
            .terminal_output(a.client_id, owner, session_id, 0)
            .is_err()
    );
    f.host
        .terminal(
            b.client_id,
            other,
            C::Create {
                cwd: None,
                cols: 80,
                rows: 24,
            },
        )
        .unwrap();
    f.host.detach_principal(other).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browser_terminal_requires_csrf_and_stream_revocation_retires_its_shells() {
    use lapis_app_protocol::terminal::TerminalCommand;
    use lapis_server::{
        control::{Control, now_ms, passwords},
        host::Principal,
    };
    let profile = tempfile::tempdir().unwrap();
    let control = Arc::new(Control::open(profile.path()).await.unwrap());
    let now = now_ms().unwrap();
    let hash = passwords::hash("terminal fixture private owner password").unwrap();
    control.initialize("Owner", &hash, now).await.unwrap();
    let issued = control.issue(now).await.unwrap();
    let principal = Principal::Session(issued.session.id);
    let f = Fixture::with_auth(Authentication::browser(control.clone(), None, true).unwrap()).await;
    let client = f.host.attach(f.ids[0], principal).unwrap();
    let url = format!("{}/api/v3/clients/{}/terminal", f.base, client.client_id);
    let cookie = format!("lapis_session={}", issued.token);
    let request = || {
        f.client
            .post(&url)
            .header("cookie", &cookie)
            .header("x-lapis-protocol", PROTOCOL_MAJOR)
            .header("origin", "http://localhost")
    };
    let command = json!({"kind":"create","cwd":null,"cols":80,"rows":24});
    assert_eq!(
        request().json(&command).send().await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request()
            .header("x-lapis-csrf", &issued.session.csrf_token)
            .header("origin", "http://foreign.invalid")
            .json(&command)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let value: serde_json::Value = request()
        .header("x-lapis-csrf", &issued.session.csrf_token)
        .json(&command)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let session = value["session_id"].as_str().unwrap();
    let mut output = f
        .client
        .get(format!("{url}/{session}/events"))
        .header("cookie", &cookie)
        .header("x-lapis-protocol", PROTOCOL_MAJOR)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert!(output.chunk().await.unwrap().is_some());
    control
        .revoke(issued.session.id, now_ms().unwrap())
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(4), async {
        while output.chunk().await.unwrap().is_some() {}
    })
    .await
    .unwrap();
    assert!(
        f.host
            .terminal(client.client_id, principal, TerminalCommand::CloseAll {})
            .is_err()
    );
    f.host.shutdown().unwrap();
    control.close().await.unwrap();
}
