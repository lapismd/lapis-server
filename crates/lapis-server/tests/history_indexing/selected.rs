use super::*;
use std::time::Duration;

fn query(text: &str) -> Value {
    json!({"kind":"query","search":{
        "expression":{"type":"term","field":"content","value":text},
        "case_sensitive":false,"cursor":null,"limit":100
    }})
}

async fn call(f: &Fixture, client: uuid::Uuid, command: Value) -> Value {
    let response = f
        .authorize(f.client.post(route(f, client)))
        .json(&command)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        200,
        "History response: {}",
        response.text().await.unwrap()
    );
    response.json().await.unwrap()
}

async fn ready(f: &Fixture, client: uuid::Uuid) {
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            let status = call(f, client, json!({"kind":"status"})).await;
            if status["complete"] == true {
                return;
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn history_indexing_selected_http_preserves_saved_identity_and_context() {
    let f = Fixture::start_with_services(true, true).await;
    std::fs::write(
        f.roots[0].path().join("Note.md"),
        "saved immutable text\nsecond line\nlast line",
    )
    .unwrap();
    let first = f.attach(f.ids[0]).await;
    ready(&f, first.client_id).await;
    let page = call(&f, first.client_id, query("saved immutable")).await;
    let hit = page["hits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|hit| hit["record"]["path"] == "Note.md")
        .unwrap();
    let commit = hit["record"]["revision"]["checkpoint"]["commit"]
        .as_str()
        .unwrap();
    assert!(!commit.is_empty());
    let context = call(
        &f,
        first.client_id,
        json!({"kind":"context","commit":commit,"path":"Note.md","start":1,"end":2}),
    )
    .await;
    assert_eq!(
        context,
        json!({"text":"second line","start":1,"end":2,"before":true,"after":true})
    );
    f.detach(&first).await;
    let next = f.attach(f.ids[0]).await;
    let retained = call(&f, next.client_id, query("saved immutable")).await;
    assert_eq!(retained["hits"], page["hits"]);
    let peer = f.attach(f.ids[1]).await;
    ready(&f, peer.client_id).await;
    let isolated = call(&f, peer.client_id, query("saved immutable")).await;
    assert!(isolated["hits"].as_array().unwrap().is_empty());
    let foreign = f
        .authorize(f.client.post(route(&f, peer.client_id)))
        .json(&json!({"kind":"context","commit":commit,"path":"Note.md","start":0,"end":1}))
        .send()
        .await
        .unwrap();
    assert!(!foreign.status().is_success());
    close(&f).await;
}

#[tokio::test]
async fn history_indexing_selected_reads_reject_replaced_physical_roots() {
    let f = Fixture::start_with_services(true, true).await;
    std::fs::write(
        f.roots[0].path().join("Note.md"),
        "retained physical history",
    )
    .unwrap();
    let client = f.attach(f.ids[0]).await;
    ready(&f, client.client_id).await;
    let original = f.roots[0].path();
    let moved = original.with_extension("retained");
    std::fs::rename(original, &moved).unwrap();
    std::fs::create_dir(original).unwrap();
    std::fs::write(original.join("Note.md"), "foreign replacement").unwrap();
    for command in [
        json!({"kind":"start"}),
        json!({"kind":"status"}),
        query("retained"),
    ] {
        let response = f
            .authorize(f.client.post(route(&f, client.client_id)))
            .json(&command)
            .send()
            .await
            .unwrap();
        assert!(!response.status().is_success());
        let value: Value = response.json().await.unwrap();
        assert!(
            !value
                .to_string()
                .contains(&original.to_string_lossy().to_string())
        );
    }
    assert_eq!(
        std::fs::read_to_string(original.join("Note.md")).unwrap(),
        "foreign replacement"
    );
    std::fs::remove_dir_all(original).unwrap();
    std::fs::rename(&moved, original).unwrap();
    close(&f).await;
}
