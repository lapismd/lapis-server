use super::*;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn passive_sync_and_recovery_routes_keep_owner_generation_and_bounded_input_admission() {
    let root = tempfile::tempdir().unwrap();
    let ids = [Uuid::new_v4(), Uuid::new_v4()];
    let f = Running::start(root.path(), None, ids).await;
    let client = f.attach(ids[0]).await;
    let url = f
        .url(&client)
        .replace("repository-remotes", "repository-synchronization");
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
    for bad in [
        url.replace(&client.generation.to_string(), &Uuid::new_v4().to_string()),
        url.replace(&client.vault_id.to_string(), &ids[1].to_string()),
    ] {
        assert_eq!(
            f.auth(f.client.get(bad)).send().await.unwrap().status(),
            403
        );
    }
    let foreign = f
        .host
        .attach(
            ids[1],
            lapis_server::host::Principal::Session(Uuid::new_v4()),
        )
        .unwrap();
    assert_eq!(
        f.auth(
            f.client.get(
                f.url(&foreign)
                    .replace("repository-remotes", "repository-synchronization")
            )
        )
        .send()
        .await
        .unwrap()
        .status(),
        403
    );
    assert_eq!(
        f.auth(f.client.get(format!("{url}&root=outside")))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let response = f.auth(f.client.get(&url)).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let status: Value = response.json().await.unwrap();
    assert_eq!(status["phase"], "paused");
    assert_eq!(status["busy"], false);
    assert_eq!(
        f.auth(f.client.post(&url))
            .json(&json!({"kind":"run", "expected_revision":0, "root":"outside"}))
            .send()
            .await
            .unwrap()
            .status(),
        422
    );
    assert_eq!(f.auth(f.client.post(&url)).json(&json!({"kind":"resolve", "expected_revision":0, "candidate":"ab", "path":"Note.md", "resolution":{"kind":"text", "text":"x".repeat(1_100_000), "executable":false}})).send().await.unwrap().status(), 413);
    assert_eq!(
        f.auth(f.client.post(&url))
            .json(&json!({"kind":"run", "expected_revision":0}))
            .send()
            .await
            .unwrap()
            .status(),
        501
    );
    f.auth(
        f.client
            .delete(format!("{}/api/v3/clients/{}", f.base, client.client_id)),
    )
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap();
    assert_eq!(
        f.auth(f.client.get(&url)).send().await.unwrap().status(),
        403
    );
    f.stop().await;
}
