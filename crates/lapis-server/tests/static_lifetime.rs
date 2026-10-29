use lapis_app_protocol::{
    ClientDescriptor, Outcome, PROTOCOL_MAJOR, Request, VaultCommand, VaultValue,
};
use lapis_server::{
    config::{Config, VaultConfig},
    host::{Host, Principal},
    http::{Authentication, StaticWeb, router_with_web},
};
use std::sync::Arc;
use uuid::Uuid;

fn execute(
    host: &Host,
    client: &ClientDescriptor,
    command: VaultCommand,
) -> lapis_app_protocol::Reply {
    host.execute(
        client.client_id,
        Principal::Token,
        Request {
            protocol_version: PROTOCOL_MAJOR,
            request_id: Uuid::new_v4(),
            vault_id: client.vault_id,
            generation: client.generation,
            command,
        },
    )
    .unwrap()
}

#[tokio::test]
async fn readiness_follows_native_shutdown_and_restores_after_a_retained_dirty_refusal() {
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
    let id = Uuid::new_v4();
    let host = Arc::new(
        Host::new(Config {
            vaults: vec![VaultConfig {
                id,
                name: "Notes".into(),
                root: vault.clone(),
            }],
        })
        .unwrap(),
    );
    let web = StaticWeb::open(&assets, &[vault]).unwrap();
    let router = router_with_web(
        host.clone(),
        Authentication::token("isolated-static-lifetime-owner-token-0123456789").unwrap(),
        "https://notes.example",
        web,
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let client = reqwest::Client::new();
    let owner = host.attach(id, Principal::Token).unwrap();
    let editor = match execute(
        &host,
        &owner,
        VaultCommand::OpenEditor {
            path: "draft.md".into(),
        },
    )
    .outcome
    {
        Outcome::Ok {
            value: VaultValue::Editor { editor_id },
        } => editor_id,
        other => panic!("{other:?}"),
    };
    assert!(matches!(
        execute(
            &host,
            &owner,
            VaultCommand::MarkDirty {
                editor_id: editor,
                revision: 1
            }
        )
        .outcome,
        Outcome::Ok { .. }
    ));
    assert!(host.shutdown().is_err());
    let ready = client.get(format!("{base}/readyz")).send().await.unwrap();
    assert_eq!(ready.status(), 200);
    assert_eq!(
        ready.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({"status":"ready"})
    );
    assert_eq!(
        client
            .get(format!("{base}/"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert!(matches!(
        execute(
            &host,
            &owner,
            VaultCommand::DiscardEditor { editor_id: editor }
        )
        .outcome,
        Outcome::Ok { .. }
    ));
    host.shutdown().unwrap();
    let ready = client.get(format!("{base}/readyz")).send().await.unwrap();
    assert_eq!(ready.status(), 503);
    assert_eq!(
        ready.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({"status":"unavailable"})
    );
    assert_eq!(
        client
            .get(format!("{base}/healthz"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .get(format!("{base}/"))
            .send()
            .await
            .unwrap()
            .status(),
        503
    );
    server.abort();
}
