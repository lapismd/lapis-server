#[path = "support/native_queue/mod.rs"]
mod support;
use lapis_app_core::services::QueuePhase;
use lapis_app_protocol::{
    ErrorCode,
    turns::{TurnContinuation, TurnPhase},
};
use std::{
    sync::{Arc, Barrier},
    time::Duration,
};
use support::{Fixture, shutdown};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepted_queue_survives_all_client_detach_and_advances_only_after_cleanup() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let mut first = f.input();
    first.prompt = "held-first".into();
    let head = client.submit_turn(first).unwrap();
    f.prompted().await;
    let second = f.queued(&head, "second");
    let accepted = client.enqueue_turn(second.clone()).unwrap();
    assert_eq!(accepted.entries[0].phase, QueuePhase::Pending);
    assert_ne!(accepted.entries[0].request_id, second.acceptance_id);
    assert_ne!(
        accepted.entries[0].request_id.to_string(),
        second.capture["user"]["id"]
    );
    assert_eq!(client.enqueue_turn(second.clone()).unwrap(), accepted);
    let mut changed = second.clone();
    changed.capture["user"]["text"] = "changed".into();
    assert_eq!(
        client.enqueue_turn(changed).unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    drop(client);
    f.release();
    // No client or waiter is needed to complete either turn.
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if f.prompt_count() == 2
                && f.transcript(&head)
                    .iter()
                    .any(|v| v["type"] == "queue.outcome")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let queue = f.queue_terminal(&runtime, head.conversation_id, 1).await;
    assert_eq!(queue.entries[0].phase, QueuePhase::Completed);
    assert!(queue.entries[0].receipt_published);
    assert_eq!(
        client.turn(head.input.request_id).unwrap().unwrap().phase,
        TurnPhase::Completed
    );
    let native = client.turn(queue.entries[0].request_id).unwrap().unwrap();
    assert_eq!(native.conversation_id, head.conversation_id);
    assert_eq!(native.input.options.effort, None);
    assert_eq!(native.input.options.mode, "ask");
    assert_eq!(
        queue.entries[0].input.capture["selection"]["targets"][0]["thinking"],
        "default"
    );
    let transcript = f.transcript(&head);
    let users = transcript
        .iter()
        .filter(|v| v["role"] == "user")
        .collect::<Vec<_>>();
    assert_eq!(users.len(), 2);
    assert_eq!(users[1], &second.capture["user"]);
    assert!(
        transcript
            .iter()
            .any(|v| v["role"] == "assistant" && v["parentId"] == second.capture["user"]["id"])
    );
    assert_eq!(
        transcript
            .iter()
            .filter(|v| v["type"] == "queue.outcome")
            .count(),
        1
    );
    let starts = f
        .commands()
        .into_iter()
        .filter(|v| v.get("pid").is_some())
        .collect::<Vec<_>>();
    assert_eq!(starts.len(), 2);
    assert_eq!(
        starts[1]["previousAlive"], false,
        "first process must be cleaned up before second starts"
    );
    let config = f
        .commands()
        .into_iter()
        .rfind(|v| v["type"] == "configure")
        .unwrap();
    // The existing private ACP wire encodes an absent typed preference as null.
    // Its receiver maps null to None and performs no provider effort call.
    assert_eq!(config["effort"], serde_json::Value::Null);
    drop(client);
    shutdown(runtime).await;
    // Simulate loss of private queue acknowledgement after proven terminal publication.
    let path = f
        .profile
        .join("queues")
        .join(f.vault.to_string())
        .join(format!("{}.json", head.conversation_id));
    let mut saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    saved["entries"][0]["phase"] = "started".into();
    saved["entries"][0]["receipt"] = serde_json::Value::Null;
    saved["entries"][0]["receipt_published"] = false.into();
    std::fs::write(path, serde_json::to_vec(&saved).unwrap()).unwrap();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let cold = client.turn_queue(head.conversation_id).unwrap().unwrap();
    assert!(cold.paused);
    assert_eq!(cold.entries[0].phase, QueuePhase::Completed);
    assert_eq!(cold.entries[0].input, second);
    assert_eq!(f.prompt_count(), 2);
    let retained = f.transcript(&head);
    assert_eq!(retained.iter().filter(|v| v["role"] == "user").count(), 2);
    assert_eq!(
        retained
            .iter()
            .find(|v| v["id"] == second.capture["user"]["id"])
            .unwrap(),
        &second.capture["user"]
    );
    assert_eq!(
        retained
            .iter()
            .filter(|v| v["type"] == "queue.outcome")
            .count(),
        1
    );
    // Explicit resume reconciles the already-published terminal receipt without replay.
    client
        .resume_turn_queue(head.conversation_id, native.input.request_id)
        .unwrap();
    assert_eq!(
        client
            .turn_queue(head.conversation_id)
            .unwrap()
            .unwrap()
            .entries[0]
            .receipt,
        queue.entries[0].receipt
    );
    assert_eq!(f.prompt_count(), 2);
    let mut next = f.input();
    next.continuation = Some(TurnContinuation {
        conversation_id: head.conversation_id,
        after_request_id: native.input.request_id,
    });
    let third = client.submit_turn(next).unwrap();
    assert_eq!(
        f.finished(&runtime, third.input.request_id).await.phase,
        TurnPhase::Completed
    );
    let prompt = f
        .commands()
        .into_iter()
        .rfind(|v| v["type"] == "prompt")
        .unwrap();
    assert!(
        !prompt["context"]
            .as_str()
            .unwrap()
            .contains("queue.outcome")
    );
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fifo_capacity_competing_clients_and_individual_cancellation_are_serialized() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let mut first = f.input();
    first.prompt = "held-first".into();
    let head = client.submit_turn(first).unwrap();
    f.prompted().await;
    let duplicate = f.queued(&head, "racing");
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let client = runtime.get(f.vault).unwrap().attach().unwrap();
        let input = duplicate.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            client.enqueue_turn(input).unwrap()
        }));
    }
    barrier.wait();
    for worker in workers {
        assert_eq!(worker.join().unwrap().entries.len(), 1);
    }
    for index in 1..16 {
        client
            .enqueue_turn(f.queued(&head, &format!("item-{index}")))
            .unwrap();
    }
    assert_eq!(
        client
            .enqueue_turn(f.queued(&head, "overflow"))
            .unwrap_err()
            .code(),
        ErrorCode::Busy
    );
    let pending = client.turn_queue(head.conversation_id).unwrap().unwrap();
    let cancelled = pending.entries[3].input.acceptance_id;
    let queue = client
        .cancel_queued_turn(head.conversation_id, cancelled)
        .unwrap();
    assert_eq!(queue.entries[3].phase, QueuePhase::Cancelled);
    assert!(
        !queue.entries[3].receipt_published,
        "idle publication must not race first turn"
    );
    client.enqueue_turn(f.queued(&head, "replacement")).unwrap();
    f.release();
    let final_queue = f.queue_terminal(&runtime, head.conversation_id, 17).await;
    assert!(final_queue.entries.iter().all(|e| e.receipt_published));
    let prompts = f
        .commands()
        .into_iter()
        .filter(|v| v["type"] == "prompt")
        .map(|v| v["text"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let expected = final_queue
        .entries
        .iter()
        .filter(|e| e.phase == QueuePhase::Completed)
        .map(|e| e.input.capture["user"]["text"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(prompts[1..], expected);
    assert_eq!(
        final_queue.entries[3].receipt.as_ref().unwrap()["outcome"],
        "cancelled"
    );
    assert_eq!(
        f.transcript(&head)
            .iter()
            .filter(|v| v["type"] == "queue.outcome")
            .count(),
        17
    );
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn encoded_byte_limit_and_rich_payload_refusal_happen_before_acceptance() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let mut first = f.input();
    first.prompt = "held-first".into();
    let head = client.submit_turn(first).unwrap();
    f.prompted().await;
    for index in 0..3 {
        let mut input = f.queued(&head, &format!("large-{index}"));
        input.capture["user"]["text"] = "x".repeat(32768).into();
        client.enqueue_turn(input).unwrap();
    }
    let mut overflow = f.queued(&head, "large-overflow");
    overflow.capture["user"]["text"] = "x".repeat(32768).into();
    assert_eq!(
        client.enqueue_turn(overflow).unwrap_err().code(),
        ErrorCode::Busy
    );
    let mut rich = f.queued(&head, "rich");
    rich.capture["selection"]["attachments"] = serde_json::json!(["secret.md"]);
    assert_eq!(
        client.enqueue_turn(rich).unwrap_err().code(),
        ErrorCode::Unsupported
    );
    let mut rich = f.queued(&head, "profile");
    rich.capture["selection"]["targets"][0]["agentProfile"] =
        serde_json::json!({"instructions":"Do not drop me"});
    assert_eq!(
        client.enqueue_turn(rich).unwrap_err().code(),
        ErrorCode::Unsupported
    );
    assert_eq!(
        client
            .turn_queue(head.conversation_id)
            .unwrap()
            .unwrap()
            .entries
            .len(),
        3
    );
    client.pause_turn_queue(head.conversation_id).unwrap();
    f.release();
    f.finished(&runtime, head.input.request_id).await;
    assert_eq!(f.prompt_count(), 1);
    drop(client);
    shutdown(runtime).await;
}
