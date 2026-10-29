#[path = "browser_auth/support.rs"]
#[allow(dead_code)]
mod browser;
#[allow(dead_code)]
mod credentials;
#[allow(dead_code)]
mod support;

use lapis_app_protocol::{ClientDescriptor, database::call::DatabaseCall};
use lapis_server::host::Principal;
use serde_json::json;
use std::time::Duration;
use support::{Fixture, ORIGIN};

fn path(f: &Fixture, client: &ClientDescriptor) -> String {
    format!(
        "{}/api/v3/clients/{}/database/events?vault_id={}&generation={}",
        f.base, client.client_id, client.vault_id, client.generation
    )
}
async fn frame(response: &mut reqwest::Response) -> Option<String> {
    tokio::time::timeout(Duration::from_secs(3), response.chunk())
        .await
        .expect("bounded observation response")
        .unwrap()
        .map(|bytes| String::from_utf8(bytes.to_vec()).unwrap())
}
async fn write(f: &Fixture, client: &ClientDescriptor, value: &str) {
    let call: DatabaseCall = serde_json::from_value(json!({
        "method":"setMeta", "args":[{"kind":"value","value":"key"},{"kind":"value","value":value}]
    }))
    .unwrap();
    f.host
        .database(client.client_id, Principal::Token, call)
        .await
        .unwrap();
}

#[tokio::test]
async fn database_observation_stream_delivers_scoped_native_commits() {
    let f = Fixture::with_database().await;
    let client = f.attach(f.ids[0]).await;
    let mut response = f
        .authorize(f.client.get(path(&f, &client)))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        200,
        "the admitted native stream is missing"
    );
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert_eq!(response.headers()["cache-control"], "no-store");
    let initial = frame(&mut response).await.unwrap();
    assert!(initial.contains("event: change") && initial.contains("\"reset\":true"));
    let foreign = f.attach(f.ids[1]).await;
    write(&f, &foreign, "foreign-vault").await;
    write(&f, &client, "owned-vault").await;
    let committed = frame(&mut response).await.unwrap();
    assert!(committed.contains("\"revision\":1") && committed.contains("\"meta\""));
    assert!(!committed.contains("foreign-vault") && !committed.contains("owned-vault"));
    f.detach(&client).await;
    while let Some(value) = frame(&mut response).await {
        assert!(!value.contains("event: change"));
    }
    let host = f.host.clone();
    tokio::task::spawn_blocking(move || host.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn stream_rejects_unadmitted_scope_protocol_and_future_cursor() {
    let f = Fixture::with_database().await;
    let client = f.attach(f.ids[0]).await;
    let url = path(&f, &client);
    assert_eq!(f.client.get(&url).send().await.unwrap().status(), 401);
    assert_eq!(
        f.client
            .get(&url)
            .bearer_auth(support::TOKEN)
            .header("x-lapis-protocol", "2")
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        f.authorize(f.client.get(&url))
            .header("origin", "https://foreign.example")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let wrong = url.replace(&client.vault_id.to_string(), &f.ids[1].to_string());
    assert_eq!(
        f.authorize(f.client.get(wrong))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let wrong = url.replace(
        &client.generation.to_string(),
        &uuid::Uuid::new_v4().to_string(),
    );
    assert_eq!(
        f.authorize(f.client.get(wrong))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.authorize(f.client.get(format!("{url}&cursor=999")))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        f.authorize(f.client.get(format!("{url}&root=/private")))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        f.authorize(f.client.get(&url))
            .header("origin", ORIGIN)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
}

#[tokio::test]
async fn reconnect_resets_to_current_without_replaying_mutations_and_close_drains() {
    let f = Fixture::with_database().await;
    let client = f.attach(f.ids[0]).await;
    write(&f, &client, "saved").await;
    let mut response = f
        .authorize(f.client.get(format!("{}&cursor=0", path(&f, &client))))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let reset = frame(&mut response).await.unwrap();
    assert!(reset.contains("\"revision\":1") && reset.contains("\"reset\":true"));
    assert!(!reset.contains("saved"));
    let host = f.host.clone();
    tokio::time::timeout(
        Duration::from_secs(3),
        tokio::task::spawn_blocking(move || host.shutdown()),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    while let Some(value) = frame(&mut response).await {
        assert!(!value.contains("event: change"));
    }
}

#[tokio::test]
async fn idle_stream_revalidates_revoked_browser_session() {
    let mut f = browser::BrowserFixture::start(false, None).await;
    let (cookie, session, _) = f.login().await;
    let client = f.attach(&cookie, &session).await;
    let url = format!(
        "{}/api/v3/clients/{}/database/events?vault_id={}&generation={}",
        f.base, client.client_id, client.vault_id, client.generation
    );
    let mut response = f
        .authorize(f.client.get(&url), &cookie, &session, false)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    frame(&mut response).await.unwrap();
    let now = lapis_server::control::now_ms().unwrap();
    let token = cookie.split_once('=').unwrap().1;
    let id = f.control.resolve(token, now).await.unwrap().unwrap().id;
    f.control.revoke(id, now).await.unwrap();
    while let Some(value) = frame(&mut response).await {
        assert!(!value.contains("event: change"));
    }
    assert_eq!(
        f.authorize(f.client.get(url), &cookie, &session, false)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    f.finish().await;
}

#[tokio::test]
async fn concurrent_observers_are_bounded_and_cannot_hold_native_drain() {
    let f = Fixture::with_database().await;
    let client = f.attach(f.ids[0]).await;
    let url = path(&f, &client);
    let mut readers = Vec::new();
    for _ in 0..64 {
        let response = f.authorize(f.client.get(&url)).send().await.unwrap();
        assert_eq!(response.status(), 200);
        readers.push(response);
    }
    assert_eq!(
        f.authorize(f.client.get(url))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    let host = f.host.clone();
    tokio::time::timeout(
        Duration::from_secs(3),
        tokio::task::spawn_blocking(move || host.shutdown()),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    drop(readers);
}
