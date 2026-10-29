mod credentials;
#[path = "support/native_participants/mod.rs"]
#[allow(dead_code)]
mod support;
use reqwest::StatusCode;
use serde_json::{Value, json};
use std::collections::HashSet;
use support::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn participant_http_retains_logical_roles_and_detached_publication() {
    let mut f = Fixture::start(false, true).await;
    peer(&f);
    let attached = f.attach(0).await;
    let input = input(4);
    let response = command(
        &f,
        attached.client_id,
        json!({"kind":"accept","input":input}),
    )
    .send()
    .await
    .unwrap();
    if response.status() != StatusCode::OK {
        let status = response.status();
        f.close().await;
        panic!("Admitted participant HTTP acceptance must succeed; got {status}");
    }
    // Drop the accepted reply body, then recover only by the original acceptance.
    drop(response);
    let accepted = call(
        &f,
        attached.client_id,
        json!({"kind":"read","acceptance_id":input["acceptance_id"]}),
    )
    .await;
    let message = &accepted["message"];
    assert_eq!(accepted["kind"], "message");
    assert_eq!(message["input"], input);
    assert_eq!(message["acceptance_id"], input["acceptance_id"]);
    let deliveries = message["deliveries"].as_array().unwrap();
    assert_eq!(deliveries.len(), 4);
    let mut native = HashSet::new();
    native.insert(input["acceptance_id"].as_str().unwrap());
    for (i, d) in deliveries.iter().enumerate() {
        assert_eq!(d["request_id"], input["turns"][i]["request_id"]);
        assert_eq!(d["delivery_id"], input["dispatch"]["deliveries"][i]["id"]);
        for key in ["request_id", "agent_binding_id", "session_id"] {
            assert!(native.insert(d[key].as_str().unwrap()));
        }
    }
    f.until(|| f.prompts() == 4).await;
    f.detach(attached.client_id).await;
    release(&f);
    f.until(|| terminal(&f, &input["acceptance_id"])).await;
    let client = f.attach(0).await;
    let read = json!({"kind":"read","acceptance_id":input["acceptance_id"]});
    let retained = call(&f, client.client_id, read.clone()).await;
    assert!(
        retained["message"]["deliveries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["state"] == "completed" && d["outcome_published"] == true)
    );
    let transcript = rows(&f, message, "transcript.jsonl");
    assert_eq!(
        transcript
            .iter()
            .filter(|r| r["role"] == "user")
            .cloned()
            .collect::<Vec<_>>(),
        [input["user"].clone()]
    );
    let assistants: Vec<_> = transcript
        .iter()
        .filter(|r| r["role"] == "assistant")
        .collect();
    assert_eq!(assistants.len(), 4);
    for r in assistants {
        assert_eq!(r["parentId"], input["user"]["id"]);
        assert!(r["text"].as_str().unwrap().ends_with(":α:β:終"));
    }
    let bindings: Vec<_> = rows(&f, message, "agents.jsonl")
        .into_iter()
        .filter(|r| r["type"] == "binding.created")
        .collect();
    assert_eq!(bindings.len(), 4);
    for (i, b) in bindings.iter().enumerate() {
        assert_eq!(b["agent"], "fixture");
        assert_eq!(
            b["participant"],
            input["dispatch"]["deliveries"][i]["recipient"]
        );
        assert_eq!(b["id"], deliveries[i]["agent_binding_id"]);
    }
    assert!(!retained.to_string().contains("not-in-wire-responses"));
    assert!(!retained.to_string().contains("executable"));
    f.restart().await;
    let client = f.attach(0).await;
    let cold = call(&f, client.client_id, read).await;
    assert_eq!(cold["message"]["recovered"], true);
    for key in ["input", "deliveries", "conversation_id", "acceptance_id"] {
        assert_eq!(cold["message"][key], retained["message"][key]);
    }
    assert_eq!(
        call(&f, client.client_id, json!({"kind":"accept","input":input})).await,
        cold
    );
    assert_eq!(f.prompts(), 4);
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn participant_cancel_matches_its_acceptance_and_preserves_started_siblings() {
    let f = Fixture::start(false, true).await;
    peer(&f);
    let client = f.attach(0).await;
    let input = input(2);
    call(&f, client.client_id, json!({"kind":"accept","input":input})).await;
    f.until(|| f.prompts() == 2).await;
    let wrong = json!({"kind":"cancel","acceptance_id":input["acceptance_id"],"request_id":uuid::Uuid::new_v4()});
    assert_eq!(
        command(&f, client.client_id, wrong)
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        command(
            &f,
            client.client_id,
            json!({"kind":"stop","acceptance_id":uuid::Uuid::new_v4()})
        )
        .send()
        .await
        .unwrap()
        .status(),
        404
    );
    let cancel = json!({"kind":"cancel","acceptance_id":input["acceptance_id"],"request_id":input["turns"][0]["request_id"]});
    assert_eq!(
        call(&f, client.client_id, cancel.clone()).await,
        json!({"kind":"cancel_requested","accepted":true})
    );
    f.until(|| {
        stored(&f, &input["acceptance_id"])
            .is_some_and(|m| m["deliveries"][0]["state"] == "unknown")
    })
    .await;
    assert_eq!(
        stored(&f, &input["acceptance_id"]).unwrap()["deliveries"][1]["state"],
        "started"
    );
    release(&f);
    f.until(|| terminal(&f, &input["acceptance_id"])).await;
    let read = call(
        &f,
        client.client_id,
        json!({"kind":"read","acceptance_id":input["acceptance_id"]}),
    )
    .await;
    assert_eq!(read["message"]["deliveries"][0]["state"], "unknown");
    assert_eq!(read["message"]["deliveries"][1]["state"], "completed");
    assert_eq!(
        call(&f, client.client_id, cancel).await,
        json!({"kind":"cancel_requested","accepted":false})
    );
    assert_eq!(
        call(&f, client.client_id, json!({"kind":"accept","input":input})).await,
        read
    );
    let mut changed = input.clone();
    changed["user"]["text"] = json!("Changed authored parent");
    let changed_text = changed["user"]["text"].clone();
    for t in changed["turns"].as_array_mut().unwrap() {
        t["prompt"] = changed_text.clone();
    }
    assert_eq!(
        command(
            &f,
            client.client_id,
            json!({"kind":"accept","input":changed})
        )
        .send()
        .await
        .unwrap()
        .status(),
        400
    );
    assert_eq!(f.prompts(), 2);
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn participant_stop_requests_every_owned_delivery_without_manufacturing_completion() {
    let f = Fixture::start(false, true).await;
    peer(&f);
    let client = f.attach(0).await;
    let input = input(4);
    call(&f, client.client_id, json!({"kind":"accept","input":input})).await;
    f.until(|| f.prompts() == 4).await;
    let stop = json!({"kind":"stop","acceptance_id":input["acceptance_id"]});
    assert_eq!(
        call(&f, client.client_id, stop.clone()).await,
        json!({"kind":"stop_requested","accepted":true})
    );
    f.until(|| terminal(&f, &input["acceptance_id"])).await;
    let read = call(
        &f,
        client.client_id,
        json!({"kind":"read","acceptance_id":input["acceptance_id"]}),
    )
    .await;
    assert!(
        read["message"]["deliveries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["state"] == "unknown")
    );
    assert_eq!(
        call(&f, client.client_id, stop).await,
        json!({"kind":"stop_requested","accepted":false})
    );
    assert_eq!(f.prompts(), 4);
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn participant_disable_drains_active_work_and_preserves_exact_retry_observation() {
    let f = Fixture::start(false, true).await;
    peer(&f);
    let client = f.attach(0).await;
    let original = input(2);
    call(
        &f,
        client.client_id,
        json!({"kind":"accept","input":original}),
    )
    .await;
    f.until(|| f.prompts() == 2).await;
    let disabled: Value = f
        .auth(f.raw(&format!("/api/v3/clients/{}/turns", client.client_id)))
        .json(&json!({"kind":"set_enabled","enabled":false}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(disabled, json!({"kind":"availability","enabled":false}));
    let read = call(
        &f,
        client.client_id,
        json!({"kind":"read","acceptance_id":original["acceptance_id"]}),
    )
    .await;
    assert!(
        read["message"]["deliveries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["state"] == "unknown")
    );
    assert_eq!(
        call(
            &f,
            client.client_id,
            json!({"kind":"accept","input":original})
        )
        .await,
        read
    );
    assert_eq!(
        command(
            &f,
            client.client_id,
            json!({"kind":"accept","input":input(2)})
        )
        .send()
        .await
        .unwrap()
        .status(),
        501
    );
    assert_eq!(f.prompts(), 2);
    f.close().await;
}
