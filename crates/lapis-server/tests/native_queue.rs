mod credentials;
#[path = "support/native_queue/mod.rs"]
mod support;
use lapis_server::http::VERSION_HEADER;
use reqwest::StatusCode;
use serde_json::json;
use support::{Fixture, ORIGIN, TOKEN};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_transport_preserves_detached_acceptance_authorship_and_cold_receipts() {
    let mut f = Fixture::start(false, true).await;
    let attached = f.attach(0).await;
    let head = f.first(attached.client_id).await;
    let input = f.accept(&head, "second");
    let response = f
        .queue(attached.client_id, input.clone())
        .send()
        .await
        .unwrap();
    if response.status() != StatusCode::OK {
        let status = response.status();
        f.release();
        f.close().await;
        panic!("Admitted native queue acceptance must succeed; got {status}");
    }
    let accepted: serde_json::Value = response.json().await.unwrap();
    assert_eq!(accepted["kind"], "queue");
    let saved = &accepted["queue"]["entries"][0];
    assert_eq!(saved["input"], input["input"]);
    assert!(saved.get("target").is_none());
    assert_eq!(
        f.call(attached.client_id, input.clone()).await,
        accepted["queue"]
    );
    let mut changed = input.clone();
    changed["input"]["capture"]["user"]["text"] = "changed".into();
    assert_eq!(
        f.queue(attached.client_id, changed)
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    f.detach(attached.client_id).await;
    f.release();
    f.until(|| {
        f.stored(head.conversation_id)
            .is_some_and(|q| q["entries"][0]["phase"] == "completed")
    })
    .await;
    assert_eq!(f.prompts(), 2);
    let starts = f
        .commands()
        .into_iter()
        .filter(|v| v.get("pid").is_some())
        .collect::<Vec<_>>();
    assert_eq!(starts[1]["previousAlive"], false);
    let config = f
        .commands()
        .into_iter()
        .rfind(|v| v["type"] == "configure")
        .unwrap();
    assert_eq!(config["mode"], "plan");
    assert_eq!(config["effort"], serde_json::Value::Null);
    f.restart().await;
    let current = f.attach(0).await;
    let read = f
        .call(
            current.client_id,
            json!({"kind":"read","conversation_id":head.conversation_id}),
        )
        .await;
    assert!(read["paused"].as_bool().unwrap());
    assert_eq!(read["entries"][0]["phase"], "completed");
    assert_eq!(read["entries"][0]["input"], input["input"]);
    assert_eq!(
        read["entries"][0]["receipt"]["queued"],
        input["input"]["capture"]
    );
    assert!(!read.to_string().contains("not-in-wire-responses"));
    assert!(read["entries"][0].get("target").is_none());
    assert_eq!(f.prompts(), 2);
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_stop_cancel_and_explicit_resume_preserve_cold_unstarted_authority() {
    let mut f = Fixture::start(false, true).await;
    let client = f.attach(0).await;
    let head = f.first(client.client_id).await;
    let a = f.accept(&head, "cancel");
    let b = f.accept(&head, "resume");
    f.call(client.client_id, a.clone()).await;
    f.call(client.client_id, b.clone()).await;
    let cancelled=f.call(client.client_id,json!({"kind":"cancel","conversation_id":head.conversation_id,"acceptance_id":a["input"]["acceptance_id"]})).await;
    assert_eq!(cancelled["entries"][0]["phase"], "cancelled");
    let stopped = f
        .call(
            client.client_id,
            json!({"kind":"stop","conversation_id":head.conversation_id}),
        )
        .await;
    assert_eq!(stopped["paused"], true);
    f.release();
    f.restart().await;
    let client = f.attach(0).await;
    let read = f
        .call(
            client.client_id,
            json!({"kind":"read","conversation_id":head.conversation_id}),
        )
        .await;
    assert_eq!(read["entries"][1]["phase"], "pending");
    assert_eq!(f.prompts(), 1);
    assert_eq!(f.queue(client.client_id,json!({"kind":"resume","conversation_id":head.conversation_id,"after_request_id":uuid::Uuid::new_v4()})).send().await.unwrap().status(),400);
    f.call(client.client_id,json!({"kind":"resume","conversation_id":head.conversation_id,"after_request_id":head.input.request_id})).await;
    f.until(|| {
        f.stored(head.conversation_id)
            .is_some_and(|q| q["entries"][1]["phase"] == "completed")
    })
    .await;
    let read = f
        .call(
            client.client_id,
            json!({"kind":"read","conversation_id":head.conversation_id}),
        )
        .await;
    assert_eq!(read["entries"][0]["phase"], "cancelled");
    assert_eq!(read["entries"][1]["input"], b["input"]);
    assert_eq!(f.prompts(), 2);
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_http_reuses_principal_origin_csrf_version_and_body_authority() {
    let f = Fixture::start(true, false).await;
    let client = f.attach(0).await;
    let path = format!("/api/v3/clients/{}/queues", client.client_id);
    let value = json!({"kind":"read","conversation_id":uuid::Uuid::new_v4()});
    assert_eq!(
        f.raw(&path).json(&value).send().await.unwrap().status(),
        401
    );
    let (cookie, session) = f.cookie.as_ref().unwrap();
    for (origin, csrf, version, status) in [
        (ORIGIN, "wrong", 3, 403),
        ("http://foreign.invalid", &session.csrf_token, 3, 403),
        (ORIGIN, &session.csrf_token, 2, 409),
    ] {
        assert_eq!(
            f.raw(&path)
                .header("cookie", cookie)
                .header("origin", origin)
                .header("x-lapis-csrf", csrf)
                .header(VERSION_HEADER, version)
                .json(&value)
                .send()
                .await
                .unwrap()
                .status(),
            status
        );
    }
    let (other, info) = f.login().await;
    assert_eq!(
        f.raw(&path)
            .header("cookie", other)
            .header("origin", ORIGIN)
            .header("x-lapis-csrf", info.csrf_token)
            .header(VERSION_HEADER, 3)
            .json(&value)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.raw(&path)
            .bearer_auth(TOKEN)
            .header(VERSION_HEADER, 3)
            .json(&value)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.queue(client.client_id, value.clone())
            .send()
            .await
            .unwrap()
            .status(),
        501
    );
    let mut unknown = value.clone();
    unknown["vault_id"] = json!(f.ids[1]);
    let invalid = f.queue(client.client_id, unknown).send().await.unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        invalid.json::<serde_json::Value>().await.unwrap()["code"],
        "INVALID_REQUEST"
    );
    let huge = "x".repeat(1024 * 1024 + 1);
    assert_eq!(
        f.auth(f.raw(&path))
            .header("content-type", "application/json")
            .body(huge)
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_input_never_selects_foreign_vault_launch_or_unsupported_capture() {
    let f = Fixture::start(false, true).await;
    let client = f.attach(0).await;
    let other = f.attach(1).await;
    let head = f.first(client.client_id).await;
    let input = f.accept(&head, "authority");
    assert!(
        f.call(
            other.client_id,
            json!({"kind":"read","conversation_id":head.conversation_id})
        )
        .await
        .is_null()
    );
    assert_eq!(
        f.queue(other.client_id, input.clone())
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    for (key, value, status) in [
        ("target_id", json!("removed-target"), 400),
        ("scope_dir", json!("../escape"), 400),
        ("executable", json!("/tmp/agent"), 422),
    ] {
        let mut forged = input.clone();
        forged["input"][key] = value;
        let invalid = f.queue(client.client_id, forged).send().await.unwrap();
        assert_eq!(invalid.status(), status);
        assert_eq!(
            invalid.json::<serde_json::Value>().await.unwrap()["code"],
            "INVALID_REQUEST"
        );
    }
    let mut rich = input.clone();
    rich["input"]["capture"]["selection"]["attachments"] = json!(["secret.md"]);
    assert_eq!(
        f.queue(client.client_id, rich)
            .send()
            .await
            .unwrap()
            .status(),
        501
    );
    assert_eq!(
        f.queue(
            client.client_id,
            json!({"kind":"read","conversation_id":uuid::Uuid::nil()})
        )
        .send()
        .await
        .unwrap()
        .status(),
        400
    );
    assert!(
        f.call(
            client.client_id,
            json!({"kind":"read","conversation_id":head.conversation_id})
        )
        .await
        .is_null()
    );
    assert_eq!(f.prompts(), 1);
    f.release();
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn admitted_targets_publish_only_complete_operator_queue_selection() {
    let f = Fixture::start(false, true).await;
    let attached = f.attach(0).await;
    let response: serde_json::Value = f
        .auth(f.raw(&format!("/api/v3/clients/{}/turns", attached.client_id)))
        .json(&json!({"kind":"targets"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        response["targets"][0],
        json!({
            "id":"fixture", "label":"Operator fixture", "default_mode":"plan",
            "queue_selection":{
                "recipient":{"id":"writer","label":"Writer","token":"jj-store-agent:777269746572;"},
                "target":{"id":"writer","agent":"local","thinking":"default"}
            }
        })
    );
    assert!(!response.to_string().contains("not-in-wire-responses"));
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raw_queue_null_target_thinking_inherits_captured_global_effort() {
    let f = Fixture::start(false, true).await;
    let client = f.attach(0).await;
    let head = f.first(client.client_id).await;
    let mut request = f.accept(&head, "null-thinking");
    request["input"]["capture"]["selection"]["targets"][0]["thinking"] = serde_json::Value::Null;
    let saved = f.call(client.client_id, request.clone()).await;
    assert_eq!(saved["entries"][0]["input"], request["input"]);
    f.release();
    f.until(|| {
        f.stored(head.conversation_id)
            .is_some_and(|q| q["entries"][0]["phase"] == "completed")
    })
    .await;
    let configuration = f
        .commands()
        .into_iter()
        .rfind(|frame| frame["type"] == "configure")
        .unwrap();
    assert_eq!(configuration["mode"], "plan");
    assert_eq!(configuration["effort"], "high");
    assert_eq!(
        f.stored(head.conversation_id).unwrap()["entries"][0]["input"],
        request["input"]
    );
    f.close().await;
}
