mod credentials;
#[path = "support/native_queue/mod.rs"]
mod support;
use lapis_app_protocol::queues::{QUEUE_RECORD_LIMIT, QUEUE_REQUEST_LIMIT};
use serde_json::{Value, json};
use support::Fixture;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_http_preserves_escaped_capture_and_exact_record_bounds() {
    let mut f = Fixture::start(false, true).await;
    let client = f.attach(0).await;
    let head = f.first(client.client_id).await;
    let mut input = f.accept(&head, "byte-boundary");
    input["input"]["capture"]["user"]["id"] = "x".into();
    let initial = serde_json::to_vec(&json!([input["input"]["capture"]]))
        .unwrap()
        .len();
    input["input"]["capture"]["user"]["id"] = "x".repeat(128 * 1024 - initial + 1).into();
    assert_eq!(
        serde_json::to_vec(&json!([input["input"]["capture"]]))
            .unwrap()
            .len(),
        128 * 1024
    );
    let body = serde_json::to_string(&input)
        .unwrap()
        .replace('x', "\\u0078");
    assert!(body.len() > 128 * 1024 && body.len() < QUEUE_REQUEST_LIMIT);
    let path = format!("/api/v3/clients/{}/queues", client.client_id);
    let accepted: Value = f
        .auth(f.raw(&path))
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(accepted["queue"]["entries"][0]["input"], input["input"]);
    f.call(client.client_id, json!({"kind":"cancel","conversation_id":head.conversation_id,"acceptance_id":input["input"]["acceptance_id"]})).await;
    f.call(
        client.client_id,
        json!({"kind":"stop","conversation_id":head.conversation_id}),
    )
    .await;
    f.release();
    f.restart().await;

    // Retained receipt extensions are opaque data. Seed one bounded cold record
    // without dispatching or granting authority through its retained fields.
    let mut stored = f.stored(head.conversation_id).unwrap();
    assert_eq!(stored["entries"][0]["phase"], "cancelled");
    stored["barrier"] = "service_restarted".into();
    stored["entries"][0]["receipt"]["retained_extension"] = "".into();
    let initial = serde_json::to_vec(&stored).unwrap().len();
    let padding = "r".repeat(QUEUE_RECORD_LIMIT - initial);
    stored["entries"][0]["receipt"]["retained_extension"] = padding.clone().into();
    let bytes = serde_json::to_vec(&stored).unwrap();
    assert_eq!(bytes.len(), QUEUE_RECORD_LIMIT);
    let record = f
        .profile
        .join("turns/queues")
        .join(f.ids[0].to_string())
        .join(format!("{}.json", head.conversation_id));
    std::fs::write(&record, &bytes).unwrap();
    let current = f.attach(0).await;
    let read = json!({"kind":"read","conversation_id":head.conversation_id});
    let reply = f
        .queue(current.client_id, read.clone())
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert!(reply.len() <= QUEUE_RECORD_LIMIT);
    let wire: Value = serde_json::from_slice(&reply).unwrap();
    assert_eq!(wire["queue"]["entries"][0]["input"], input["input"]);
    assert_eq!(
        wire["queue"]["entries"][0]["receipt"]["retained_extension"],
        padding
    );
    assert!(wire["queue"]["entries"][0].get("target").is_none());
    let mut oversized = bytes.clone();
    oversized.push(b' ');
    std::fs::write(&record, &oversized).unwrap();
    let rejected = f.queue(current.client_id, read).send().await.unwrap();
    assert_eq!(rejected.status(), 400);
    assert_eq!(
        rejected.json::<Value>().await.unwrap()["code"],
        "INVALID_REQUEST"
    );
    assert_eq!(std::fs::read(&record).unwrap(), oversized);
    std::fs::write(&record, bytes).unwrap();
    assert_eq!(f.prompts(), 1);
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_http_revocation_detach_and_close_match_existing_turn_admission() {
    let mut f = Fixture::start(true, true).await;
    let first = f.attach(0).await;
    let second = f.attach(0).await;
    let head = f.first(first.client_id).await;
    let input = f.accept(&head, "revocation");
    f.call(first.client_id, input.clone()).await;
    let read = json!({"kind":"read","conversation_id":head.conversation_id});
    f.detach(first.client_id).await;
    assert_eq!(
        f.queue(first.client_id, read.clone())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let logout = f.auth(f.raw("/api/v3/auth/logout")).send();
    let observe = f.queue(second.client_id, read.clone()).send();
    let (logout, observe) = tokio::join!(logout, observe);
    assert!(logout.unwrap().status().is_success());
    let observe = observe.unwrap();
    // Authentication and attachment retirement can win in either order. A call
    // admitted first may finish; later calls never reacquire the retired client.
    assert!(matches!(observe.status().as_u16(), 200 | 401 | 403));
    if observe.status().is_success() {
        assert_eq!(
            observe.json::<Value>().await.unwrap()["queue"]["entries"][0]["input"],
            input["input"]
        );
    }
    assert_eq!(
        f.queue(second.client_id, read.clone())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    f.cookie = Some(f.login().await);
    for client in [first.client_id, second.client_id] {
        assert_eq!(
            f.queue(client, read.clone()).send().await.unwrap().status(),
            403
        );
        assert_eq!(
            f.auth(f.raw(&format!("/api/v3/clients/{client}/turns")))
                .json(&json!({"kind":"list"}))
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
    }
    let current = f.attach(0).await;
    assert_eq!(
        f.call(current.client_id, read.clone()).await["entries"][0]["input"],
        input["input"]
    );
    f.release();
    f.until(|| {
        f.stored(head.conversation_id)
            .is_some_and(|q| q["entries"][0]["phase"] == "completed")
    })
    .await;
    assert_eq!(f.prompts(), 2);
    let host = f.host.clone();
    let vault = f.ids[0];
    tokio::task::spawn_blocking(move || host.close(vault))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        f.queue(current.client_id, read)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.auth(f.raw(&format!("/api/v3/clients/{}/turns", current.client_id)))
            .json(&json!({"kind":"list"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    f.close().await;
}
