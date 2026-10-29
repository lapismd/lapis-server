use super::support::{Fixture, TOKEN};
use serde_json::{Value, json};

fn route(f: &Fixture, client: uuid::Uuid, method: &str) -> String {
    format!("{}/api/v3/clients/{client}/history/{method}", f.base)
}
#[tokio::test]
async fn repository_reader_reuses_authentication_and_rejects_foreign_authority() {
    let f = Fixture::with_database().await;
    let attached = f.attach(f.ids[0]).await;
    let url = route(&f, attached.client_id, "heads");
    let frame = json!({"version":1,"data":{"vault":attached.vault_id}}).to_string();
    assert_eq!(
        f.client
            .post(&url)
            .body(frame.clone())
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
            .body(frame.clone())
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        f.authorize(f.client.post(&url))
            .header("origin", "https://foreign.invalid")
            .body(frame.clone())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    for (method, input, expected) in [
        ("step", frame.clone(), 400),
        (
            "heads",
            json!({"version":2,"data":{"vault":attached.vault_id}}).to_string(),
            400,
        ),
        (
            "heads",
            json!({"version":1,"data":{"vault":f.ids[1]}}).to_string(),
            403,
        ),
        (
            "heads",
            json!({"version":1,"data":{"vault":attached.vault_id,"root":"/outside"}}).to_string(),
            400,
        ),
    ] {
        let response = f
            .authorize(f.client.post(route(&f, attached.client_id, method)))
            .body(input)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        assert!(
            response
                .json::<Value>()
                .await
                .unwrap()
                .get("history")
                .is_some()
        );
    }
    let response = f
        .authorize(f.client.post(&url))
        .body(frame.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response.json::<Value>().await.unwrap()["data"]["heads"]
            .as_array()
            .is_some()
    );
    f.detach(&attached).await;
    assert_eq!(
        f.authorize(f.client.post(&url))
            .body(frame)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let host = f.host.clone();
    tokio::task::spawn_blocking(move || host.shutdown())
        .await
        .unwrap()
        .unwrap();
}
