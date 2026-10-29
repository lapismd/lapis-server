use crate::support::Fixture;
use serde_json::{Value, json};

fn route(f: &Fixture, client: uuid::Uuid) -> String {
    format!("{}/api/v3/clients/{client}/history-index", f.base)
}

async fn close(f: &Fixture) {
    let host = f.host.clone();
    tokio::task::spawn_blocking(move || host.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn history_indexing_keeps_native_selection_explicit() {
    let f = Fixture::with_database().await;
    std::fs::write(f.roots[0].path().join("Note.md"), "saved hosted history").unwrap();
    let attached = f.attach(f.ids[0]).await;
    let url = route(&f, attached.client_id);
    for command in [json!({"kind":"status"}), json!({"kind":"start"})] {
        let response = f
            .authorize(f.client.post(&url))
            .json(&command)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 501);
        assert_eq!(
            response.json::<Value>().await.unwrap()["code"],
            "UNSUPPORTED"
        );
    }
    let history = f.profile.as_ref().unwrap().path().join("vault-history");
    assert!(
        !history
            .join(f.ids[0].to_string())
            .join("history-search-v2")
            .exists()
    );
    assert_eq!(
        std::fs::read_to_string(f.roots[0].path().join("Note.md")).unwrap(),
        "saved hosted history"
    );
    close(&f).await;
}

#[tokio::test]
async fn history_indexing_refuses_foreign_authority_and_renderer_scheduling() {
    let f = Fixture::with_database().await;
    let attached = f.attach(f.ids[0]).await;
    let url = route(&f, attached.client_id);
    let status = json!({"kind":"status"});
    let unauthorized = f
        .client
        .post(&url)
        .header("x-lapis-protocol", "3")
        .json(&status)
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), 401);
    let foreign = f
        .authorize(f.client.post(&url))
        .header("origin", "https://foreign.invalid")
        .json(&status)
        .send()
        .await
        .unwrap();
    assert_eq!(foreign.status(), 403);
    for command in [
        json!({"kind":"step"}),
        json!({"kind":"start","root":"/outside"}),
        json!({"kind":"status","exclusions":["private"]}),
        json!({"kind":"status","window":"another-client"}),
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
    for command in [
        json!({"kind":"query","search":{"expression":{"type":"term","field":null,"value":"note"},"case_sensitive":false,"cursor":null,"limit":101}}),
        json!({"kind":"context","commit":"abcd","path":"../outside","start":0,"end":1}),
        json!({"kind":"context","commit":"abcd","path":"Note.md","start":0,"end":81}),
    ] {
        let response = f
            .authorize(f.client.post(&url))
            .json(&command)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
        assert_eq!(
            response.json::<Value>().await.unwrap()["code"],
            "INVALID_REQUEST"
        );
    }
    close(&f).await;
}

#[tokio::test]
async fn history_indexing_fences_detached_clients_without_selecting_peer_work() {
    let f = Fixture::with_database().await;
    let old = f.attach(f.ids[0]).await;
    let peer = f.attach(f.ids[1]).await;
    f.detach(&old).await;
    let retired = f
        .authorize(f.client.post(route(&f, old.client_id)))
        .json(&json!({"kind":"status"}))
        .send()
        .await
        .unwrap();
    assert_eq!(retired.status(), 403);
    assert_eq!(
        retired.json::<Value>().await.unwrap()["code"],
        "PERMISSION_DENIED"
    );
    let remaining = f
        .authorize(f.client.post(route(&f, peer.client_id)))
        .json(&json!({"kind":"status"}))
        .send()
        .await
        .unwrap();
    assert_eq!(remaining.status(), 501);
    assert_eq!(
        remaining.json::<Value>().await.unwrap()["code"],
        "UNSUPPORTED"
    );
    close(&f).await;
}
mod selected;
