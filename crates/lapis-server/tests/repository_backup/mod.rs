use super::support::{Fixture, TOKEN};
use serde_json::{Value, json};
use std::fs;
use uuid::Uuid;

fn route(f: &Fixture, client: Uuid) -> String {
    format!("{}/api/v3/clients/{client}/repository-backup", f.base)
}
async fn invoke(f: &Fixture, client: Uuid, input: Value) -> reqwest::Response {
    f.authorize(f.client.post(route(f, client)))
        .json(&input)
        .send()
        .await
        .unwrap()
}
async fn status(f: &Fixture, client: Uuid) -> Value {
    invoke(f, client, json!({"kind":"status"}))
        .await
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saved_disable_is_honored_before_attach_and_explicit_controls_preserve_other_settings() {
    let f = Fixture::with_database().await;
    let root = f.roots[0].path();
    fs::create_dir(root.join(".lapis")).unwrap();
    let path = root.join(".lapis/app.json");
    let original = json!({"keep":"retained","pluginData":{"other":{"value":2},"repository-history":{"automaticBackup":false,"showWorkingCopyStatus":false}}});
    fs::write(&path, original.to_string()).unwrap();
    let c = f.attach(f.ids[0]).await;
    let first = status(&f, c.client_id).await;
    assert_eq!(first["enabled"], false);
    assert_eq!(first["status"]["phase"], "closed");
    assert!(
        !f.profile
            .as_ref()
            .unwrap()
            .path()
            .join("vault-history")
            .join(f.ids[0].to_string())
            .exists()
    );
    let response = invoke(
        &f,
        c.client_id,
        json!({"kind":"set_enabled","enabled":true,"expected":first["configuration"]}),
    )
    .await;
    assert_eq!(response.status(), 200);
    let ready: Value = response.json().await.unwrap();
    assert_eq!(ready["status"]["phase"], "ready");
    assert!(ready["status"]["checkpoint"]["commit"].is_string());
    fs::write(root.join("Note.md"), "manual capture").unwrap();
    assert_eq!(
        invoke(&f, c.client_id, json!({"kind":"capture"}))
            .await
            .status(),
        200
    );
    assert_eq!(
        invoke(
            &f,
            c.client_id,
            json!({"kind":"set_enabled","enabled":false,"expected":ready["configuration"]})
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        serde_json::from_str::<Value>(&fs::read_to_string(&path).unwrap()).unwrap(),
        original
    );
    f.detach(&c).await;
    let later = f.attach(f.ids[0]).await;
    assert_eq!(
        status(&f, later.client_id).await["status"]["phase"],
        "closed"
    );
    assert_eq!(
        invoke(&f, later.client_id, json!({"kind":"capture"}))
            .await
            .status(),
        501
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selection_refuses_stale_configuration_and_keeps_vaults_separate() {
    let f = Fixture::with_database().await;
    let a = f.attach(f.ids[0]).await;
    let b = f.attach(f.ids[1]).await;
    let first = status(&f, a.client_id).await;
    fs::create_dir_all(f.roots[0].path().join(".lapis")).unwrap();
    let path = f.roots[0].path().join(".lapis/app.json");
    fs::write(&path, r#"{"keep":"external"}"#).unwrap();
    assert_eq!(
        invoke(
            &f,
            a.client_id,
            json!({"kind":"set_enabled","enabled":false,"expected":first["configuration"]})
        )
        .await
        .status(),
        409
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), r#"{"keep":"external"}"#);
    let current = status(&f, a.client_id).await;
    invoke(
        &f,
        a.client_id,
        json!({"kind":"set_enabled","enabled":false,"expected":current["configuration"]}),
    )
    .await
    .error_for_status()
    .unwrap();
    assert_eq!(status(&f, a.client_id).await["enabled"], false);
    assert_eq!(status(&f, b.client_id).await["enabled"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn backup_controls_are_bounded_authenticated_and_admission_scoped() {
    let f = Fixture::with_database().await;
    let c = f.attach(f.ids[0]).await;
    let url = route(&f, c.client_id);
    let input = json!({"kind":"status"});
    assert_eq!(
        f.client
            .post(&url)
            .json(&input)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        f.client
            .post(&url)
            .bearer_auth(TOKEN)
            .json(&input)
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        f.authorize(f.client.post(&url))
            .header("origin", "https://foreign.invalid")
            .json(&input)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    for value in [
        json!({"kind":"status","root":"/outside"}),
        json!({"kind":"set_enabled","enabled":false}),
        json!({"kind":"capture","vault":"other"}),
    ] {
        assert_eq!(invoke(&f, c.client_id, value).await.status(), 422);
    }
    assert_eq!(invoke(&f,c.client_id,json!({"kind":"set_enabled","enabled":false,"expected":{"kind":"version","version":"a".repeat(20_000)}})).await.status(),413);
    let foreign = f
        .host
        .attach(
            f.ids[1],
            lapis_server::host::Principal::Session(Uuid::new_v4()),
        )
        .unwrap();
    assert_eq!(
        invoke(&f, foreign.client_id, input.clone()).await.status(),
        403
    );
    f.detach(&c).await;
    assert_eq!(invoke(&f, c.client_id, input).await.status(), 403);
}
