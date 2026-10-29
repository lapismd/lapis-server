#![cfg(unix)]

use lapis_app_protocol::{
    AttachClient, ClientDescriptor, ExpectedVersion, Outcome, PROTOCOL_MAJOR, Reply, Request,
    VaultCommand, VaultValue,
};
use lapis_server::http::VERSION_HEADER;
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    time::timeout,
};
use uuid::Uuid;

const TOKEN: &str = "isolated-cold-server-owner-token-0123456789";

async fn start(config: &Path) -> (Child, String) {
    start_selected(config, None).await
}
async fn start_selected(config: &Path, agents: Option<(&Path, &Path)>) -> (Child, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lapis-server"));
    command
        .arg(config)
        .arg("127.0.0.1:0")
        .env("LAPIS_SERVER_TOKEN", TOKEN)
        .env_remove("LAPIS_SERVER_ORIGIN")
        .env_remove("LAPIS_SERVER_DATA_ROOT")
        .env_remove("LAPIS_SERVER_AGENTS_CONFIG")
        .env_remove("LAPIS_SERVER_BOOTSTRAP_USERNAME")
        .env_remove("LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE")
        .env_remove("LAPIS_SERVER_INSECURE_COOKIE")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    if let Some((profile, settings)) = agents {
        use std::os::unix::fs::PermissionsExt;
        let password = profile.join("bootstrap-password");
        std::fs::write(&password, "isolated-native-turn-owner-password").unwrap();
        std::fs::set_permissions(&password, std::fs::Permissions::from_mode(0o600)).unwrap();
        command
            .env("LAPIS_SERVER_DATA_ROOT", profile)
            .env("LAPIS_SERVER_AGENTS_CONFIG", settings)
            .env("LAPIS_SERVER_BOOTSTRAP_USERNAME", "owner")
            .env("LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE", password)
            .env("LAPIS_SERVER_ORIGIN", "https://native-turn.fixture");
    }
    let mut child = command.spawn().unwrap();
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

#[tokio::test]
async fn sigterm_has_a_bounded_network_drain_for_a_stalled_file_consumer() {
    let root = tempfile::tempdir().unwrap();
    let vault = root.path().join("vault");
    std::fs::create_dir(&vault).unwrap();
    let file = std::fs::File::create(vault.join("large.bin")).unwrap();
    file.set_len(64 * 1024 * 1024).unwrap();
    let id = Uuid::new_v4();
    let config = root.path().join("server.json");
    std::fs::write(
        &config,
        serde_json::to_vec(
            &serde_json::json!({"vaults": [{"id": id, "name": "Slow reader", "root": vault}]}),
        )
        .unwrap(),
    )
    .unwrap();
    let client = reqwest::Client::new();
    let (mut child, base) = start(&config).await;
    let attached = attach(&client, &base, id).await;
    let mut socket = tokio::net::TcpStream::connect(base.strip_prefix("http://").unwrap())
        .await
        .unwrap();
    socket.write_all(format!("GET /api/v3/clients/{}/files/large.bin HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN}\r\nx-lapis-protocol: 3\r\n\r\n", attached.client_id).as_bytes()).await.unwrap();
    let mut headers = Vec::new();
    timeout(Duration::from_secs(10), async {
        while !headers.ends_with(b"\r\n\r\n") {
            headers.push(socket.read_u8().await.unwrap());
        }
    })
    .await
    .unwrap();
    assert!(headers.starts_with(b"HTTP/1.1 200"));
    // Retain the socket without consuming the large response body.
    signal(&child).await;
    assert!(
        timeout(Duration::from_secs(12), child.wait())
            .await
            .expect("server waited for the network consumer after its native leave gate passed")
            .unwrap()
            .success()
    );
    drop(socket);
}
async fn signal(child: &Child) {
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(child.id().unwrap().to_string())
        .status()
        .await
        .unwrap();
    assert!(status.success());
}
async fn attach(client: &reqwest::Client, base: &str, vault: Uuid) -> ClientDescriptor {
    client
        .post(format!("{base}/api/v3/vaults/{vault}/clients"))
        .bearer_auth(TOKEN)
        .header(VERSION_HEADER, PROTOCOL_MAJOR)
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
    command: VaultCommand,
) -> Reply {
    client
        .post(format!(
            "{base}/api/v3/clients/{}/commands",
            attached.client_id
        ))
        .bearer_auth(TOKEN)
        .header(VERSION_HEADER, PROTOCOL_MAJOR)
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
async fn cold_binary_preserves_files_and_blocks_sigterm_until_dirty_claim_is_resolved() {
    let root = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let vault = root.path().join("vault");
    std::fs::create_dir(&vault).unwrap();
    let config = root.path().join("server.json");
    std::fs::write(
        &config,
        serde_json::to_vec(
            &serde_json::json!({"vaults": [{"id": id, "name": "Cold Notes", "root": vault}]}),
        )
        .unwrap(),
    )
    .unwrap();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let (mut child, base) = start(&config).await;
    let attached = attach(&client, &base, id).await;
    let saved = execute(
        &client,
        &base,
        &attached,
        VaultCommand::WriteText {
            path: "note.md".into(),
            text: "# persisted 💎".into(),
            expected: ExpectedVersion::Missing {},
        },
    )
    .await;
    assert!(matches!(saved.outcome, Outcome::Ok { .. }));
    let editor = match execute(
        &client,
        &base,
        &attached,
        VaultCommand::OpenEditor {
            path: "note.md".into(),
        },
    )
    .await
    .outcome
    {
        Outcome::Ok {
            value: VaultValue::Editor { editor_id },
        } => editor_id,
        other => panic!("{other:?}"),
    };
    assert!(matches!(
        execute(
            &client,
            &base,
            &attached,
            VaultCommand::MarkDirty {
                editor_id: editor,
                revision: 1
            }
        )
        .await
        .outcome,
        Outcome::Ok { .. }
    ));
    signal(&child).await;
    assert!(
        timeout(Duration::from_millis(250), child.wait())
            .await
            .is_err()
    );
    assert!(matches!(
        execute(&client, &base, &attached, VaultCommand::Status {})
            .await
            .outcome,
        Outcome::Ok { .. }
    ));
    assert!(matches!(
        execute(
            &client,
            &base,
            &attached,
            VaultCommand::DiscardEditor { editor_id: editor }
        )
        .await
        .outcome,
        Outcome::Ok { .. }
    ));
    signal(&child).await;
    assert!(
        timeout(Duration::from_secs(5), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    let (mut child, base) = start(&config).await;
    let fresh = attach(&client, &base, id).await;
    assert_ne!(fresh.generation, attached.generation);
    assert!(
        matches!(execute(&client, &base, &fresh, VaultCommand::ReadText { path: "note.md".into() }).await.outcome, Outcome::Ok { value: VaultValue::Text { snapshot } } if snapshot.text == "# persisted 💎")
    );
    let stale = client
        .post(format!(
            "{base}/api/v3/clients/{}/commands",
            attached.client_id
        ))
        .bearer_auth(TOKEN)
        .header(VERSION_HEADER, PROTOCOL_MAJOR)
        .json(&Request {
            protocol_version: PROTOCOL_MAJOR,
            request_id: Uuid::new_v4(),
            vault_id: id,
            generation: attached.generation,
            command: VaultCommand::Status {},
        })
        .send()
        .await
        .unwrap();
    assert_eq!(stale.status(), 403);
    signal(&child).await;
    assert!(
        timeout(Duration::from_secs(5), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
}

#[tokio::test]
async fn configured_binary_finishes_detached_native_work_and_preserves_it_after_restart() {
    use lapis_app_protocol::turns::{TurnInput, TurnPhase, TurnRecord};
    use std::{fs, os::unix::fs::PermissionsExt};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let vault = root.join("vault");
    let profile = root.join("profile");
    fs::create_dir(&vault).unwrap();
    fs::create_dir(&profile).unwrap();
    let helper = root.join("peer.py");
    let log = root.join("worker.jsonl");
    fs::write(
        &helper,
        include_str!("../../lapis-app-core/tests/support/native_turn/peer.py"),
    )
    .unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
    let id = Uuid::new_v4();
    let config = root.join("server.json");
    let agents = root.join("agents.json");
    fs::write(
        &config,
        serde_json::to_vec(
            &serde_json::json!({"vaults":[{"id":id,"name":"Native Notes","root":vault}]}),
        )
        .unwrap(),
    )
    .unwrap();
    fs::write(&agents,serde_json::to_vec(&serde_json::json!([{"id":"fixture","label":"Fixture","default_mode":"ask","executable":helper,"environment":{"LOG":log}}])).unwrap()).unwrap();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let (mut child, base) = start_selected(&config, Some((&profile, &agents))).await;
    let attached = attach(&client, &base, id).await;
    let input = TurnInput {
        request_id: Uuid::new_v4(),
        continuation: None,
        target_id: "fixture".into(),
        scope_dir: "".into(),
        prompt: "Binary-owned native turn".into(),
        context: None,
        options: Default::default(),
    };
    let call = |base: &str, client_id: Uuid, value: serde_json::Value| {
        client
            .post(format!("{base}/api/v3/clients/{client_id}/turns"))
            .bearer_auth(TOKEN)
            .header(VERSION_HEADER, PROTOCOL_MAJOR)
            .json(&value)
    };
    let accepted: serde_json::Value = call(
        &base,
        attached.client_id,
        serde_json::json!({"kind":"submit","input":input}),
    )
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap()
    .json()
    .await
    .unwrap();
    client
        .delete(format!("{base}/api/v3/clients/{}", attached.client_id))
        .bearer_auth(TOKEN)
        .header(VERSION_HEADER, PROTOCOL_MAJOR)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let journal = profile
        .join("turns")
        .join(id.to_string())
        .join(format!("{}.json", input.request_id));
    let completed = timeout(Duration::from_secs(8), async {
        loop {
            let bytes = fs::read(&journal).unwrap();
            let record: TurnRecord = serde_json::from_slice(&bytes).unwrap();
            if !matches!(record.phase, TurnPhase::Accepted | TurnPhase::Running) {
                break record;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(completed.phase, TurnPhase::Completed);
    assert_eq!(
        accepted["record"]["conversation_id"],
        completed.conversation_id.to_string()
    );
    signal(&child).await;
    assert!(
        timeout(Duration::from_secs(8), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    let (mut child, base) = start_selected(&config, Some((&profile, &agents))).await;
    let fresh = attach(&client, &base, id).await;
    let read: serde_json::Value = call(
        &base,
        fresh.client_id,
        serde_json::json!({"kind":"read","request_id":input.request_id}),
    )
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(read["record"], serde_json::to_value(completed).unwrap());
    let duplicate: serde_json::Value = call(
        &base,
        fresh.client_id,
        serde_json::json!({"kind":"submit","input":input}),
    )
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(duplicate, read);
    assert_eq!(
        fs::read_to_string(log)
            .unwrap()
            .lines()
            .filter(
                |line| serde_json::from_str::<serde_json::Value>(line).unwrap()["type"] == "prompt"
            )
            .count(),
        1
    );
    signal(&child).await;
    assert!(
        timeout(Duration::from_secs(8), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
}
