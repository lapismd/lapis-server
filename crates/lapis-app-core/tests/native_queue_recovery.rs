#[path = "support/native_queue/mod.rs"]
mod support;
use lapis_app_core::services::{QueuePhase, TurnQueue};
use lapis_app_protocol::{ErrorCode, turns::TurnCommand};
use std::{
    fs,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use support::{Fixture, shutdown};

async fn paused(
    f: &Fixture,
    runtime: &lapis_app_core::services::Runtime,
    id: uuid::Uuid,
) -> TurnQueue {
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let queue = client.turn_queue(id).unwrap().unwrap();
            if queue.paused {
                return queue;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_native_cancel_and_disable_fence_queue_advancement_before_cleanup() {
    for action in ["stop", "cancel", "disable"] {
        let f = Fixture::new();
        let runtime = f.runtime();
        let client = Arc::new(runtime.get(f.vault).unwrap().attach().unwrap());
        let mut first = f.input();
        first.prompt = "held-first".into();
        let head = client.submit_turn(first).unwrap();
        f.prompted().await;
        let input = f.queued(&head, action);
        client.enqueue_turn(input.clone()).unwrap();
        match action {
            "stop" => {
                client.pause_turn_queue(head.conversation_id).unwrap();
            }
            "cancel" => {
                client
                    .call_turn(TurnCommand::Cancel {
                        request_id: head.input.request_id,
                    })
                    .unwrap();
            }
            _ => {
                let stopping = client.clone();
                tokio::task::spawn_blocking(move || stopping.set_turns_enabled(false))
                    .await
                    .unwrap()
                    .unwrap();
            }
        }
        f.release();
        f.finished(&runtime, head.input.request_id).await;
        let queue = client.turn_queue(head.conversation_id).unwrap().unwrap();
        assert!(queue.paused);
        assert_eq!(queue.entries[0].phase, QueuePhase::Pending);
        assert_eq!(f.prompt_count(), 1);
        assert_eq!(
            client.enqueue_turn(input).unwrap(),
            queue,
            "lost acceptance reply stays queryable after Stop"
        );
        client.set_turns_enabled(true).unwrap();
        client
            .resume_turn_queue(head.conversation_id, head.input.request_id)
            .unwrap();
        assert_eq!(
            f.queue_terminal(&runtime, head.conversation_id, 1)
                .await
                .entries[0]
                .phase,
            QueuePhase::Completed
        );
        assert_eq!(f.prompt_count(), 2);
        drop(client);
        shutdown(runtime).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn current_operator_target_and_default_metadata_are_revalidated_at_dispatch() {
    for changed in [false, true] {
        let f = Fixture::new();
        let runtime = f.runtime();
        let client = runtime.get(f.vault).unwrap().attach().unwrap();
        let mut first = f.input();
        first.prompt = "held-first".into();
        let head = client.submit_turn(first).unwrap();
        f.prompted().await;
        f.policy.missing.store(true, Ordering::SeqCst);
        assert_eq!(
            client
                .enqueue_turn(f.queued(&head, "unavailable"))
                .unwrap_err()
                .code(),
            ErrorCode::InvalidRequest
        );
        assert!(client.turn_queue(head.conversation_id).unwrap().is_none());
        f.policy.missing.store(false, Ordering::SeqCst);
        let input = f.queued(&head, "target");
        client.enqueue_turn(input.clone()).unwrap();
        if changed {
            f.policy.changed.store(true, Ordering::SeqCst);
        } else {
            f.policy.missing.store(true, Ordering::SeqCst);
        }
        f.release();
        f.finished(&runtime, head.input.request_id).await;
        let queue = paused(&f, &runtime, head.conversation_id).await;
        assert_eq!(queue.entries[0].phase, QueuePhase::Pending);
        assert_eq!(f.prompt_count(), 1);
        assert_eq!(client.enqueue_turn(input).unwrap(), queue);
        f.policy.missing.store(false, Ordering::SeqCst);
        f.policy.changed.store(false, Ordering::SeqCst);
        client
            .resume_turn_queue(head.conversation_id, head.input.request_id)
            .unwrap();
        assert_eq!(
            f.queue_terminal(&runtime, head.conversation_id, 1)
                .await
                .entries[0]
                .phase,
            QueuePhase::Completed
        );
        drop(client);
        shutdown(runtime).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_unknown_and_provider_cancelled_turns_are_barriers_until_explicit_resume() {
    for outcome in ["denied", "held-unknown", "held-cancelled"] {
        let f = Fixture::new();
        let runtime = f.runtime();
        let client = runtime.get(f.vault).unwrap().attach().unwrap();
        let mut first = f.input();
        first.prompt = "held-first".into();
        let head = client.submit_turn(first).unwrap();
        f.prompted().await;
        let mut second = f.queued(&head, "barrier");
        if outcome == "denied" {
            second.capture["selection"]["targets"][0]["executionMode"] = "denied".into();
        } else {
            second.capture["user"]["text"] = outcome.into();
        }
        client.enqueue_turn(second).unwrap();
        client
            .enqueue_turn(f.queued(&head, "after-barrier"))
            .unwrap();
        f.release();
        let queue = paused(&f, &runtime, head.conversation_id).await;
        let expected = match outcome {
            "denied" => QueuePhase::Rejected,
            "held-unknown" => QueuePhase::Unknown,
            _ => QueuePhase::Cancelled,
        };
        assert_eq!(queue.entries[0].phase, expected);
        assert_eq!(queue.entries[1].phase, QueuePhase::Pending);
        assert_eq!(f.prompt_count(), if outcome == "denied" { 1 } else { 2 });
        assert_eq!(
            client
                .resume_turn_queue(head.conversation_id, head.input.request_id)
                .unwrap_err()
                .code(),
            ErrorCode::InvalidRequest
        );
        client
            .resume_turn_queue(head.conversation_id, queue.head_request_id)
            .unwrap();
        let final_queue = f.queue_terminal(&runtime, head.conversation_id, 2).await;
        assert_eq!(final_queue.entries[0].phase, expected);
        assert_eq!(final_queue.entries[1].phase, QueuePhase::Completed);
        assert_eq!(f.prompt_count(), if outcome == "denied" { 2 } else { 3 });
        drop(client);
        shutdown(runtime).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_cold_pauses_unstarted_entries_and_requires_an_exact_explicit_head() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let mut first = f.input();
    first.prompt = "held-first".into();
    let head = client.submit_turn(first).unwrap();
    f.prompted().await;
    let second = f.queued(&head, "cold");
    client.enqueue_turn(second.clone()).unwrap();
    drop(client);
    shutdown(runtime).await;
    assert_eq!(f.prompt_count(), 1);
    f.release();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let restored = client.turn_queue(head.conversation_id).unwrap().unwrap();
    assert!(restored.paused);
    assert_eq!(restored.entries[0].phase, QueuePhase::Pending);
    assert_eq!(f.prompt_count(), 1);
    assert_eq!(
        client
            .resume_turn_queue(head.conversation_id, uuid::Uuid::new_v4())
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    client
        .resume_turn_queue(head.conversation_id, head.input.request_id)
        .unwrap();
    let queue = f.queue_terminal(&runtime, head.conversation_id, 1).await;
    assert_eq!(queue.entries[0].input, second);
    assert_eq!(queue.entries[0].phase, QueuePhase::Completed);
    assert_eq!(f.prompt_count(), 2);
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dirty_history_and_scope_admission_refuse_before_acceptance_and_preserve_external_bytes() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let head = client.submit_turn(f.input()).unwrap();
    f.finished(&runtime, head.input.request_id).await;
    let mut wrong = f.queued(&head, "wrong-scope");
    wrong.scope_dir = "../escape".into();
    assert!(client.enqueue_turn(wrong).is_err());
    let path = format!(
        ".lapis/agents/sessions/{}/transcript.jsonl",
        head.conversation_id
    );
    let editor = client.open_editor(&path).unwrap();
    client.mark_dirty(editor, 1).unwrap();
    assert_eq!(
        client
            .enqueue_turn(f.queued(&head, "dirty"))
            .unwrap_err()
            .code(),
        ErrorCode::Busy
    );
    assert!(client.turn_queue(head.conversation_id).unwrap().is_none());
    let version = client.read_text(&path).unwrap().version;
    client.mark_saved(editor, 1, &version).unwrap();
    client.close_editor(editor).unwrap();
    let mut held = f.queued(&head, "publication");
    held.capture["user"]["text"] = "held-queued".into();
    let queue = client.enqueue_turn(held).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while f.prompt_count() < 2 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    client
        .enqueue_turn(f.queued(
            &client.turn(queue.entries[0].request_id).unwrap().unwrap(),
            "after-conflict",
        ))
        .unwrap();
    fs::write(
        f.conversation(&head, "transcript.jsonl"),
        "external bytes preserved",
    )
    .unwrap();
    fs::write(f._temp.path().join("release.queued"), b"ready").unwrap();
    let queue = paused(&f, &runtime, head.conversation_id).await;
    assert_eq!(queue.entries[0].phase, QueuePhase::Unknown);
    assert_eq!(queue.entries[1].phase, QueuePhase::Pending);
    assert_eq!(
        fs::read_to_string(f.conversation(&head, "transcript.jsonl")).unwrap(),
        "external bytes preserved"
    );
    assert_eq!(f.prompt_count(), 2);
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cold_started_delivery_is_never_replayed_and_only_unstarted_entries_resume() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let mut first = f.input();
    first.prompt = "held-first".into();
    let head = client.submit_turn(first).unwrap();
    f.prompted().await;
    let mut second = f.queued(&head, "started");
    second.capture["user"]["text"] = "held-queued".into();
    client.enqueue_turn(second).unwrap();
    client.enqueue_turn(f.queued(&head, "unstarted")).unwrap();
    f.release();
    tokio::time::timeout(Duration::from_secs(5), async {
        while f.prompt_count() < 2 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let before = client.turn_queue(head.conversation_id).unwrap().unwrap();
    assert_eq!(before.entries[0].phase, QueuePhase::Started);
    assert_eq!(
        client
            .cancel_queued_turn(head.conversation_id, before.entries[0].input.acceptance_id)
            .unwrap_err()
            .code(),
        ErrorCode::Busy
    );
    drop(client);
    shutdown(runtime).await;
    // Model a crash after portable/journal finish but before queue acknowledgement.
    // The real started delivery was cleaned up above; no process is left running.
    let path = f
        .profile
        .join("queues")
        .join(f.vault.to_string())
        .join(format!("{}.json", head.conversation_id));
    let mut retained: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    retained["entries"][0]["phase"] = "started".into();
    retained["entries"][0]["receipt"] = serde_json::Value::Null;
    retained["entries"][0]["receipt_published"] = false.into();
    fs::write(path, serde_json::to_vec(&retained).unwrap()).unwrap();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let restored = client.turn_queue(head.conversation_id).unwrap().unwrap();
    assert!(restored.paused);
    assert_eq!(restored.entries[0].phase, QueuePhase::Unknown);
    assert_eq!(restored.entries[1].phase, QueuePhase::Pending);
    assert_eq!(f.prompt_count(), 2);
    client
        .resume_turn_queue(head.conversation_id, restored.head_request_id)
        .unwrap();
    let queue = f.queue_terminal(&runtime, head.conversation_id, 2).await;
    assert_eq!(queue.entries[0].phase, QueuePhase::Unknown);
    assert_eq!(queue.entries[1].phase, QueuePhase::Completed);
    assert_eq!(f.prompt_count(), 3);
    assert_eq!(
        f.transcript(&head)
            .iter()
            .filter(|v| v["type"] == "queue.outcome")
            .count(),
        2
    );
    let followup = f.queued(
        &client.turn(queue.head_request_id).unwrap().unwrap(),
        "after-resume",
    );
    client.enqueue_turn(followup).unwrap();
    f.queue_terminal(&runtime, head.conversation_id, 3).await;
    assert_eq!(f.prompt_count(), 4);
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_authored_identity_must_not_alias_portable_entries_or_its_own_receipt() {
    let mut accepted_aliases = Vec::new();
    for alias in ["existing", "receipt"] {
        let f = Fixture::new();
        let runtime = f.runtime();
        let client = runtime.get(f.vault).unwrap().attach().unwrap();
        let mut first = f.input();
        first.prompt = "held-first".into();
        let head = client.submit_turn(first).unwrap();
        f.prompted().await;
        let mut input = f.queued(&head, alias);
        input.capture["user"]["id"] = match alias {
            "existing" => head.input.request_id.to_string(),
            _ => format!("queue:{}:completed", input.capture["id"].as_str().unwrap()),
        }
        .into();
        let accepted = client.enqueue_turn(input).is_ok();
        if accepted {
            accepted_aliases.push(alias);
        }
        client
            .call_turn(TurnCommand::Cancel {
                request_id: head.input.request_id,
            })
            .unwrap();
        f.release();
        f.finished(&runtime, head.input.request_id).await;
        drop(client);
        shutdown(runtime).await;
    }
    assert!(
        accepted_aliases.is_empty(),
        "Native admission accepted authored identity aliases: {accepted_aliases:?}"
    );
}
