mod search;
use crate::support::Fixture;
use serde_json::{Value, json};

#[tokio::test]
async fn indexing_controls_keep_host_selection_explicit_and_fence_only_generated_writes() {
    let f = Fixture::with_database().await;
    std::fs::write(f.roots[0].path().join("Note.md"), "# HTTP indexing\n").unwrap();
    let attached = f.attach(f.ids[0]).await;
    let url = format!("{}/api/v3/clients/{}/indexing", f.base, attached.client_id);
    let status = f
        .authorize(f.client.post(&url))
        .json(&json!({"kind":"status"}))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), 200);
    assert!(status.json::<Value>().await.unwrap()["status"].is_null());
    for command in [
        json!({"kind":"select_metadata"}),
        json!({"kind":"refresh","rebuild":false}),
    ] {
        let response = f
            .authorize(f.client.post(&url))
            .json(&command)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let value = response.json::<Value>().await.unwrap();
        assert!(value["status"]["error"].is_null(), "{value}");
    }
    let database = format!("{}/api/v3/clients/{}/database", f.base, attached.client_id);
    let response = f
        .authorize(f.client.post(&database))
        .json(&json!({"method":"deleteIndexedFile","args":[{"kind":"value","value":"Note.md"}]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    assert_eq!(
        response.json::<Value>().await.unwrap()["code"],
        "PERMISSION_DENIED"
    );
    let response = f
        .authorize(f.client.post(&database))
        .json(&json!({"method":"getIndexedFile","args":[{"kind":"value","value":"Note.md"}]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<Value>().await.unwrap()["value"]["file"]["path"],
        "Note.md"
    );
    f.detach(&attached).await;
    let peer = f.attach(f.ids[0]).await;
    let response = f
        .authorize(f.client.post(format!(
            "{}/api/v3/clients/{}/indexing",
            f.base, peer.client_id
        )))
        .json(&json!({"kind":"status"}))
        .send()
        .await
        .unwrap();
    assert!(
        response.json::<Value>().await.unwrap()["status"]["passes"]
            .as_u64()
            .unwrap()
            > 0
    );
    let other = f.attach(f.ids[1]).await;
    let response = f
        .authorize(f.client.post(format!(
            "{}/api/v3/clients/{}/indexing",
            f.base, other.client_id
        )))
        .json(&json!({"kind":"status"}))
        .send()
        .await
        .unwrap();
    assert!(response.json::<Value>().await.unwrap()["status"].is_null());
    let host = f.host.clone();
    tokio::task::spawn_blocking(move || host.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn indexing_http_admission_rejects_foreign_origins_callbacks_and_oversized_declarations() {
    let f = Fixture::with_database().await;
    let attached = f.attach(f.ids[0]).await;
    let url = format!("{}/api/v3/clients/{}/indexing", f.base, attached.client_id);
    assert_eq!(
        f.client
            .post(&url)
            .header("x-lapis-protocol", "3")
            .json(&json!({"kind":"status"}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        f.authorize(f.client.post(&url))
            .header("origin", "https://foreign.invalid")
            .json(&json!({"kind":"status"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    for command in [
        json!({"kind":"select_metadata","root":"/outside"}),
        json!({"kind":"register_projection","contribution":{"projection_id":"docs/note","owner_plugin_id":"docs","schema_version":1,"configuration_hash":"","path_prefix":"docs","project":"function(){}"}}),
    ] {
        let response = f
            .authorize(f.client.post(&url))
            .json(&command)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 422);
        assert_eq!(
            response.json::<Value>().await.unwrap()["code"],
            "INVALID_REQUEST"
        );
    }
    let response = f.authorize(f.client.post(&url)).json(&json!({"kind":"register_processor","owner_plugin_id":"x".repeat(20_000),"extension":"md"})).send().await.unwrap();
    assert_eq!(response.status(), 413);
    let host = f.host.clone();
    tokio::task::spawn_blocking(move || host.shutdown())
        .await
        .unwrap()
        .unwrap();
}
