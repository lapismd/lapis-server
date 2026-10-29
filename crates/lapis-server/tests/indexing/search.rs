use crate::support::Fixture;
use serde_json::{Value, json};

async fn control(f: &Fixture, url: &str, command: Value) -> Value {
    let response = f
        .authorize(f.client.post(url))
        .json(&command)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    response.json().await.unwrap()
}

#[tokio::test]
async fn compiled_search_controls_use_the_same_owner_over_authenticated_http() {
    let f = Fixture::with_database().await;
    std::fs::write(f.roots[0].path().join("Note.md"), "# Hosted\n").unwrap();
    std::fs::write(
        f.roots[0].path().join("Diagram.canvas"),
        r#"{"nodes":[{"text":"pear"}]}"#,
    )
    .unwrap();
    let attached = f.attach(f.ids[0]).await;
    let url = format!("{}/api/v3/clients/{}/indexing", f.base, attached.client_id);
    let selected = control(
        &f,
        &url,
        json!({"kind":"select_search","selection":{
            "providers":[
                {"provider_id":"search:markdown","version":"1","priority":0},
                {"provider_id":"search:canvas","version":"1","priority":0}
            ],
            "chunking":{"targetChars":256,"breakpointWindowChars":32,"breakpointDecay":0.5}
        }}),
    )
    .await;
    assert_eq!(selected["contribution"]["placement"], "native");
    f.detach(&attached).await;
    let peer = f.attach(f.ids[0]).await;
    let url = format!("{}/api/v3/clients/{}/indexing", f.base, peer.client_id);
    let rebuilt = control(&f, &url, json!({"kind":"refresh","rebuild":true})).await;
    assert!(rebuilt["status"]["error"].is_null(), "{rebuilt}");
    assert!(rebuilt["status"]["completed_at"].is_number());
    let database = format!("{}/api/v3/clients/{}/database", f.base, peer.client_id);
    let row = control(
        &f,
        &database,
        json!({"method":"getSearchDocument","args":[{"kind":"value","value":"Diagram.canvas"}]}),
    )
    .await;
    assert_eq!(row["value"]["content"], "pear");
    let refused = f
        .authorize(f.client.post(&database))
        .json(&json!({"method":"deleteSearchDocument","args":[{"kind":"value","value":"Diagram.canvas"}]}))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 403);
    control(&f, &url, json!({"kind":"disable_search"})).await;
    control(
        &f,
        &database,
        json!({"method":"deleteSearchDocument","args":[{"kind":"value","value":"Diagram.canvas"}]}),
    )
    .await;
    control(&f, &url, json!({"kind":"refresh","rebuild":false})).await;
    let removed = control(
        &f,
        &database,
        json!({"method":"getSearchDocument","args":[{"kind":"value","value":"Diagram.canvas"}]}),
    )
    .await;
    assert_eq!(removed["kind"], "undefined");
    let host = f.host.clone();
    tokio::task::spawn_blocking(move || host.shutdown())
        .await
        .unwrap()
        .unwrap();
}
