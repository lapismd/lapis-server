#![cfg(unix)]

use lapis_app_protocol::{
    AttachClient, ClientDescriptor, ExpectedVersion, Outcome, PROTOCOL_MAJOR, Reply, Request,
    VaultCommand, VaultValue, browser::SessionInfo,
};
use lapis_server::http::VERSION_HEADER;
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    time::timeout,
};
use uuid::Uuid;

const ORIGIN: &str = "http://localhost";
const PASSWORD: &str = "correct horse battery staple";
async fn start(config: &Path, profile: &Path, password: Option<&Path>) -> (Child, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lapis-server"));
    command
        .arg(config)
        .arg("127.0.0.1:0")
        .env_remove("LAPIS_SERVER_TOKEN")
        .env("LAPIS_SERVER_DATA_ROOT", profile)
        .env("LAPIS_SERVER_ORIGIN", ORIGIN)
        .env("LAPIS_SERVER_INSECURE_COOKIE", "1")
        .env_remove("LAPIS_SERVER_BOOTSTRAP_USERNAME")
        .env_remove("LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE");
    if let Some(password) = password {
        command
            .env("LAPIS_SERVER_BOOTSTRAP_USERNAME", "Owner")
            .env("LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE", password);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
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
async fn stop(child: &mut Child) {
    assert!(
        Command::new("kill")
            .arg("-TERM")
            .arg(child.id().unwrap().to_string())
            .status()
            .await
            .unwrap()
            .success()
    );
    assert!(
        timeout(Duration::from_secs(5), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
}
fn authorize(
    builder: reqwest::RequestBuilder,
    cookie: &str,
    info: &SessionInfo,
) -> reqwest::RequestBuilder {
    builder
        .header("cookie", cookie)
        .header(VERSION_HEADER, PROTOCOL_MAJOR)
        .header("origin", ORIGIN)
        .header("x-lapis-csrf", &info.csrf_token)
}
async fn attach(
    client: &reqwest::Client,
    base: &str,
    id: Uuid,
    cookie: &str,
    info: &SessionInfo,
) -> ClientDescriptor {
    authorize(
        client.post(format!("{base}/api/v3/vaults/{id}/clients")),
        cookie,
        info,
    )
    .json(&AttachClient {
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
async fn execute(
    client: &reqwest::Client,
    base: &str,
    attached: &ClientDescriptor,
    cookie: &str,
    info: &SessionInfo,
    command: VaultCommand,
) -> Reply {
    authorize(
        client.post(format!(
            "{base}/api/v3/clients/{}/commands",
            attached.client_id
        )),
        cookie,
        info,
    )
    .json(&Request {
        protocol_version: PROTOCOL_MAJOR,
        request_id: Uuid::new_v4(),
        vault_id: attached.vault_id,
        generation: attached.generation,
        command,
    })
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap()
}

#[tokio::test]
async fn real_rust_host_bootstraps_once_and_restores_cookie_sessions_and_notes_after_restart() {
    let root = tempfile::tempdir().unwrap();
    let vault = root.path().join("vault");
    std::fs::create_dir(&vault).unwrap();
    let profile = root.path().join("profile");
    let password = root.path().join("bootstrap-password");
    std::fs::write(&password, format!("{PASSWORD}\n")).unwrap();
    let id = Uuid::new_v4();
    let config = root.path().join("server.json");
    std::fs::write(
        &config,
        serde_json::to_vec(
            &serde_json::json!({"vaults":[{"id":id,"name":"Cookie Notes","root":vault}]}),
        )
        .unwrap(),
    )
    .unwrap();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let (mut child, base) = start(&config, &profile, Some(&password)).await;
    let response = client
        .post(format!("{base}/api/v3/auth/login"))
        .header("origin", ORIGIN)
        .header(VERSION_HEADER, PROTOCOL_MAJOR)
        .json(&serde_json::json!({"username":"owner","password":PASSWORD}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let info: SessionInfo = response.json().await.unwrap();
    let original = attach(&client, &base, id, &cookie, &info).await;
    let saved = execute(
        &client,
        &base,
        &original,
        &cookie,
        &info,
        VaultCommand::WriteText {
            path: "note.md".into(),
            text: "# Saved 💎".into(),
            expected: ExpectedVersion::Missing {},
        },
    )
    .await;
    assert!(matches!(saved.outcome, Outcome::Ok { .. }));
    let stored = authorize(
        client.post(format!("{base}/api/v3/clients/{}/database", original.client_id)),
        &cookie,
        &info,
    )
    .json(&serde_json::json!({"method":"setMeta","args":[{"kind":"value","value":"cold"},{"kind":"value","value":"retained"}]}))
    .send().await.unwrap();
    assert_eq!(stored.status(), 200);
    assert_eq!(
        stored.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({"kind":"undefined"})
    );
    stop(&mut child).await;
    std::fs::remove_file(&password).unwrap();
    use std::os::unix::fs::PermissionsExt;
    for name in ["databases", "models", "vault-history"] {
        assert_eq!(
            std::fs::metadata(profile.join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    assert_eq!(
        std::fs::metadata(profile.join("control"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(profile.join("control/auth.turso"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let (mut child, base) = start(&config, &profile, None).await;
    let current = authorize(
        client.get(format!("{base}/api/v3/auth/current")),
        &cookie,
        &info,
    )
    .send()
    .await
    .unwrap();
    assert_eq!(current.status(), 200);
    assert_eq!(
        current.json::<SessionInfo>().await.unwrap().csrf_token,
        info.csrf_token
    );
    let fresh = attach(&client, &base, id, &cookie, &info).await;
    assert_ne!(fresh.generation, original.generation);
    let stored = authorize(
        client.post(format!(
            "{base}/api/v3/clients/{}/database",
            fresh.client_id
        )),
        &cookie,
        &info,
    )
    .json(&serde_json::json!({"method":"getMeta","args":[{"kind":"value","value":"cold"}]}))
    .send()
    .await
    .unwrap();
    assert_eq!(stored.status(), 200);
    assert_eq!(
        stored.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({"kind":"value","value":"retained"})
    );
    let read = execute(
        &client,
        &base,
        &fresh,
        &cookie,
        &info,
        VaultCommand::ReadText {
            path: "note.md".into(),
        },
    )
    .await;
    assert!(
        matches!(read.outcome,Outcome::Ok {value:VaultValue::Text {snapshot}} if snapshot.text=="# Saved 💎")
    );
    assert_eq!(
        authorize(
            client.post(format!("{base}/api/v3/auth/logout")),
            &cookie,
            &info
        )
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
    assert_eq!(
        authorize(
            client.get(format!("{base}/api/v3/auth/current")),
            &cookie,
            &info
        )
        .send()
        .await
        .unwrap()
        .status(),
        401
    );
    stop(&mut child).await;
}
