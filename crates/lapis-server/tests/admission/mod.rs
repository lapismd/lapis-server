use super::support::{Fixture, ORIGIN, TOKEN};
use lapis_app_protocol::{ErrorCode, Outcome, PROTOCOL_MAJOR, Request, ServiceError, VaultCommand};
use lapis_server::http::VERSION_HEADER;
use uuid::Uuid;

#[tokio::test]
async fn authentication_version_and_origin_reject_before_admission() {
    let f = Fixture::start().await;
    let url = format!("{}/api/v3/vaults", f.base);
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
    assert_eq!(f.client.get(&url).send().await.unwrap().status(), 401);
    assert_eq!(
        f.client
            .get(&url)
            .bearer_auth("incorrect")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let stale = f
        .client
        .get(&url)
        .bearer_auth(TOKEN)
        .header(VERSION_HEADER, 2)
        .send()
        .await
        .unwrap();
    assert_eq!(stale.status(), 409);
    assert_eq!(
        stale.json::<ServiceError>().await.unwrap().code,
        ErrorCode::ProtocolMismatch
    );
    assert_eq!(
        f.authorize(f.client.get(&url))
            .header("origin", "https://untrusted.invalid")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let response = f
        .authorize(f.client.get(&url))
        .header("origin", ORIGIN)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(
        !response
            .headers()
            .contains_key("access-control-allow-origin")
    );
    let text = response.text().await.unwrap();
    assert!(!text.contains(f.roots[0].path().to_str().unwrap()));
    let vaults: Vec<lapis_app_protocol::ManagedVault> = serde_json::from_str(&text).unwrap();
    assert_eq!(vaults.len(), 2);
    f.host.shutdown().unwrap();
}

#[tokio::test]
async fn caller_cannot_choose_roots_native_actions_or_foreign_generations() {
    let f = Fixture::start().await;
    let url = format!("{}/api/v3/vaults/{}/clients", f.base, f.ids[0]);
    let stale = f
        .authorize(f.client.post(&url))
        .json(&serde_json::json!({"protocol_version": 2}))
        .send()
        .await
        .unwrap();
    assert_eq!(stale.status(), 409);
    let invalid = f
        .authorize(f.client.post(url))
        .json(&serde_json::json!({"protocol_version": 3, "root": f.roots[1].path()}))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 422);
    let unknown = f
        .authorize(f.client.post(format!(
            "{}/api/v3/vaults/{}/clients",
            f.base,
            Uuid::new_v4()
        )))
        .json(&serde_json::json!({"protocol_version": 3}))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown.status(), 403);
    let client = f.attach(f.ids[0]).await;
    for (vault_id, generation, version, request_id) in [
        (f.ids[1], client.generation, PROTOCOL_MAJOR, Uuid::new_v4()),
        (f.ids[0], Uuid::new_v4(), PROTOCOL_MAJOR, Uuid::new_v4()),
        (f.ids[0], client.generation, 2, Uuid::new_v4()),
        (f.ids[0], client.generation, PROTOCOL_MAJOR, Uuid::nil()),
    ] {
        let reply: lapis_app_protocol::Reply = f
            .authorize(f.client.post(format!(
                "{}/api/v3/clients/{}/commands",
                f.base, client.client_id
            )))
            .json(&Request {
                protocol_version: version,
                request_id,
                vault_id,
                generation,
                command: VaultCommand::Status {},
            })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(matches!(reply.outcome, Outcome::Error { .. }));
    }
    let invalid = f.authorize(f.client.post(format!("{}/api/v3/clients/{}/commands", f.base, client.client_id)))
        .json(&serde_json::json!({"protocol_version": 3, "request_id": Uuid::new_v4(), "vault_id": f.ids[0], "generation": client.generation, "command": {"kind": "files", "operation": {"operation": "open-path", "path": "note.md"}}})).send().await.unwrap();
    assert_eq!(invalid.status(), 422);
    let invalid = f
        .command(
            &client,
            VaultCommand::ReadText {
                path: "../outside".into(),
            },
        )
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 400);
    let text = invalid.text().await.unwrap();
    assert!(!text.contains(f.roots[0].path().to_str().unwrap()));
    f.detach(&client).await;
    let detached = f
        .command(&client, VaultCommand::Status {})
        .send()
        .await
        .unwrap();
    assert_eq!(detached.status(), 403);
    f.host.shutdown().unwrap();
}

#[tokio::test]
async fn oversized_json_is_rejected_before_any_file_write() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    let response = f
        .command(
            &client,
            VaultCommand::WriteText {
                path: "large.md".into(),
                text: "x".repeat(lapis_server::http::JSON_LIMIT),
                expected: lapis_app_protocol::ExpectedVersion::Missing {},
            },
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
    assert!(!f.roots[0].path().join("large.md").exists());
    f.detach(&client).await;
    f.host.shutdown().unwrap();
}
