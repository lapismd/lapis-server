#[path = "native_participant_cases/lifecycle.rs"]
mod lifecycle;
#[path = "support/native_participants/mod.rs"]
mod support;
use lapis_app_core::services::ParticipantState;
use lapis_app_protocol::turns::{TurnContinuation, TurnPhase};
use serde_json::json;
use std::fs;
use support::{
    Fixture, first, input, peer, prompted, release, restore, rows, settled, shutdown, snapshot,
    stored, until,
};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_native_participants_keep_distinct_chunks_and_independent_unknown_outcomes() {
    let f = Fixture::new();
    peer(&f);
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let mut input = input(&f, 3);
    for (turn, model) in input
        .turns
        .iter_mut()
        .zip(["held-left", "held-right", "held-unknown"])
    {
        turn.options.model = Some(model.into());
    }
    let accepted = client.submit_participant_message(input.clone()).unwrap();
    drop(client);
    prompted(&f, 3).await;
    assert!(
        stored(&f, input.acceptance_id)
            .deliveries
            .iter()
            .all(|d| d.state == ParticipantState::Started)
    );
    release(&f, None);
    let done = settled(&f, input.acceptance_id).await;
    assert_eq!(done.input, input);
    assert_eq!(
        done.deliveries.iter().map(|d| d.state).collect::<Vec<_>>(),
        [
            ParticipantState::Completed,
            ParticipantState::Completed,
            ParticipantState::Unknown
        ]
    );
    assert!(done.deliveries.iter().all(|d| d.outcome_published));
    let entries = rows(&f, &done, "transcript.jsonl");
    assert_eq!(
        entries
            .iter()
            .filter(|v| v["id"] == input.user["id"])
            .count(),
        1
    );
    for (index, delivery) in done.deliveries.iter().enumerate() {
        let text = format!(
            "{}:α:β:終",
            input.turns[index].options.model.as_ref().unwrap()
        );
        assert!(entries.iter().any(|v| v["role"] == "assistant"
            && v["agentBindingId"] == delivery.agent_binding_id.to_string()
            && v["text"] == text));
    }
    assert_eq!(
        first(&f, &runtime, &accepted).await.phase,
        TurnPhase::Completed
    );
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn participant_external_transcript_binding_and_metadata_edits_fence_later_publication() {
    for name in ["transcript.jsonl", "agents.jsonl", "metadata.yaml"] {
        let f = Fixture::new();
        peer(&f);
        let runtime = f.runtime();
        let client = runtime.get(f.vault).unwrap().attach().unwrap();
        let mut input = input(&f, 2);
        for turn in &mut input.turns {
            turn.options.model = Some("held-output".into());
        }
        let accepted = client.submit_participant_message(input.clone()).unwrap();
        drop(client);
        prompted(&f, 2).await;
        let path = f
            .root
            .join(".lapis/agents/sessions")
            .join(accepted.conversation_id.to_string())
            .join(name);
        let external = format!("external owner edit in {name}\n");
        fs::write(&path, &external).unwrap();
        release(&f, None);
        let done = settled(&f, input.acceptance_id).await;
        assert_eq!(done.input, input);
        assert!(
            done.deliveries
                .iter()
                .all(|d| d.state == ParticipantState::Unknown && !d.outcome_published),
            "{name}: {done:?}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), external);
        shutdown(runtime).await;
        assert_eq!(fs::read_to_string(path).unwrap(), external);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cold_participant_checkpoint_retains_confirmed_output_and_started_unknown_without_replay() {
    let f = Fixture::new();
    peer(&f);
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let mut input = input(&f, 3);
    for (turn, model) in input
        .turns
        .iter_mut()
        .zip(["held-first", "held-second", "held-third"])
    {
        turn.options.model = Some(model.into());
    }
    client.submit_participant_message(input.clone()).unwrap();
    drop(client);
    prompted(&f, 3).await;
    release(&f, Some("held-first"));
    until(|| stored(&f, input.acceptance_id).deliveries[0].state == ParticipantState::Completed)
        .await;
    let checkpoint = stored(&f, input.acceptance_id);
    assert_eq!(checkpoint.deliveries[1].state, ParticipantState::Started);
    let mut files = snapshot(&f.profile);
    files.extend(snapshot(&f.root));
    release(&f, None);
    settled(&f, input.acceptance_id).await;
    shutdown(runtime).await;
    restore(files);
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let portable = snapshot(&f.root);
    let recovered = client
        .participant_for_turn(input.turns[0].request_id)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot(&f.root), portable);
    assert_eq!(recovered.input, input);
    assert!(recovered.recovered);
    assert_eq!(
        recovered
            .deliveries
            .iter()
            .map(|d| d.state)
            .collect::<Vec<_>>(),
        [
            ParticipantState::Completed,
            ParticipantState::Unknown,
            ParticipantState::Unknown
        ]
    );
    assert_eq!(
        recovered.deliveries,
        checkpoint
            .deliveries
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let mut d = d.clone();
                if i > 0 {
                    d.state = ParticipantState::Unknown;
                    d.outcome_published = false;
                }
                d
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(
        client.submit_participant_message(input.clone()).unwrap(),
        recovered
    );
    assert_eq!(
        f.commands()
            .iter()
            .filter(|v| v["type"] == "prompt")
            .count(),
        3
    );
    let mut next = support::input(&f, 1);
    next.user["id"] = "next-authored".into();
    next.dispatch["userEntryId"] = "next-authored".into();
    next.dispatch["id"] = "next-dispatch".into();
    next.dispatch["deliveries"][0]["id"] = "next-delivery".into();
    next.turns[0].continuation = Some(TurnContinuation {
        conversation_id: recovered.conversation_id,
        after_request_id: input.turns[2].request_id,
    });
    client.submit_participant_message(next.clone()).unwrap();
    let continued = settled(&f, next.acceptance_id).await;
    assert_eq!(continued.deliveries[0].state, ParticipantState::Completed);
    assert_eq!(
        client
            .participant_message(input.acceptance_id)
            .unwrap()
            .unwrap(),
        recovered
    );
    let entries = rows(&f, &recovered, "transcript.jsonl");
    assert_eq!(
        entries
            .iter()
            .filter(|v| v["delivery"]["state"] == "completed")
            .count(),
        2
    );
    for delivery in &recovered.deliveries[1..] {
        assert_eq!(
            entries
                .iter()
                .filter(|v| v["id"] == format!("delivery:{}:unknown", delivery.delivery_id))
                .count(),
            1
        );
    }
    assert_eq!(
        f.commands()
            .iter()
            .filter(|v| v["type"] == "prompt")
            .count(),
        4
    );
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "current_thread")]
async fn cold_accepted_participants_become_not_sent_and_existing_receipts_never_create_claims() {
    let f = Fixture::new();
    peer(&f);
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let input = input(&f, 2);
    let accepted = client.submit_participant_message(input.clone()).unwrap();
    assert!(
        stored(&f, input.acceptance_id)
            .deliveries
            .iter()
            .all(|d| d.state == ParticipantState::Pending)
    );
    assert!(!f.commands().iter().any(|v| v["type"] == "prompt"));
    let mut files = snapshot(&f.profile);
    files.extend(snapshot(&f.root));
    drop(client);
    settled(&f, input.acceptance_id).await;
    shutdown(runtime).await;
    restore(files);
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let portable = snapshot(&f.root);
    let recovered = client
        .participant_for_turn(input.turns[0].request_id)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot(&f.root), portable);
    assert!(
        recovered
            .deliveries
            .iter()
            .all(|d| d.state == ParticipantState::NotSent && !d.outcome_published)
    );
    assert_eq!(recovered.input, input);
    assert_eq!(recovered.conversation_id, accepted.conversation_id);
    assert_eq!(
        client.submit_participant_message(input.clone()).unwrap(),
        recovered
    );
    assert_eq!(
        f.commands()
            .iter()
            .filter(|v| v["type"] == "prompt")
            .count(),
        2
    );
    let mut next = f.input();
    next.context = None;
    next.continuation = Some(TurnContinuation {
        conversation_id: recovered.conversation_id,
        after_request_id: input.turns[1].request_id,
    });
    let accepted = client.submit_turn(next).unwrap();
    assert_eq!(
        f.finished(&runtime, accepted.input.request_id).await.phase,
        TurnPhase::Completed
    );
    let entries = rows(&f, &recovered, "transcript.jsonl");
    assert_eq!(
        entries
            .iter()
            .filter(|v| v["delivery"]["state"] == "not_sent")
            .count(),
        2
    );
    assert_eq!(
        entries
            .iter()
            .filter(|v| v["id"] == input.user["id"])
            .count(),
        1
    );
    let bindings = rows(&f, &recovered, "agents.jsonl");
    assert!(bindings.last().unwrap().get("participant").is_none());
    assert_eq!(
        f.commands()
            .iter()
            .filter(|v| v["type"] == "prompt")
            .count(),
        3
    );
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn four_same_target_participants_retain_exact_retries_and_refuse_identity_or_capture_changes()
{
    let f = Fixture::new();
    peer(&f);
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let input = input(&f, 4);
    let accepted = client.submit_participant_message(input.clone()).unwrap();
    settled(&f, input.acceptance_id).await;
    let done = client
        .participant_message(input.acceptance_id)
        .unwrap()
        .unwrap();
    assert!(
        done.deliveries
            .iter()
            .all(|d| d.state == ParticipantState::Completed)
    );
    assert_eq!(
        client.submit_participant_message(input.clone()).unwrap(),
        done
    );
    let mut changed = input.clone();
    changed.user["text"] = "changed".into();
    for turn in &mut changed.turns {
        turn.prompt = "changed".into();
    }
    assert!(client.submit_participant_message(changed).is_err());
    let mut reused = input.clone();
    reused.acceptance_id = Uuid::new_v4();
    assert!(client.submit_participant_message(reused.clone()).is_err());
    assert!(
        client
            .participant_message(reused.acceptance_id)
            .unwrap()
            .is_none()
    );
    let mut follow = input.clone();
    follow.acceptance_id = Uuid::new_v4();
    follow.dispatch["id"] = "another-dispatch".into();
    for turn in &mut follow.turns {
        turn.request_id = Uuid::new_v4();
        turn.continuation = Some(TurnContinuation {
            conversation_id: accepted.conversation_id,
            after_request_id: input.turns[3].request_id,
        });
    }
    assert!(client.submit_participant_message(follow).is_err());
    let bindings = rows(&f, &done, "agents.jsonl");
    assert_eq!(bindings.len(), 4);
    for (index, delivery) in done.deliveries.iter().enumerate() {
        assert!(
            bindings
                .iter()
                .any(|b| b["id"] == delivery.agent_binding_id.to_string()
                    && b["participant"] == input.user["recipients"][index])
        );
    }
    assert_eq!(
        f.commands()
            .iter()
            .filter(|v| v["type"] == "prompt")
            .count(),
        4
    );
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejected_participant_configuration_does_not_replace_a_completed_sibling() {
    let f = Fixture::new();
    peer(&f);
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let mut input = input(&f, 2);
    input.turns[0].options.mode = "denied".into();
    client.submit_participant_message(input.clone()).unwrap();
    until(|| {
        stored(&f, input.acceptance_id)
            .deliveries
            .iter()
            .all(|d| d.state != ParticipantState::Pending)
    })
    .await;
    fs::write(
        format!("{}.configure.release", f.policy.log.display()),
        b"ready",
    )
    .unwrap();
    let done = settled(&f, input.acceptance_id).await;
    assert_eq!(done.deliveries[0].state, ParticipantState::Rejected);
    assert_eq!(done.deliveries[1].state, ParticipantState::Completed);
    assert!(done.deliveries.iter().all(|d| d.outcome_published));
    assert_eq!(
        f.commands()
            .iter()
            .filter(|v| v["type"] == "prompt")
            .count(),
        1
    );
    assert!(
        rows(&f, &done, "transcript.jsonl")
            .iter()
            .any(|v| v["delivery"]["state"] == "rejected"
                && v["delivery"]["id"] == done.deliveries[0].delivery_id)
    );
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_participant_capture_or_native_scope_refuses_before_acceptance_and_prompt() {
    let f = Fixture::new();
    peer(&f);
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    for index in 0..7 {
        let mut value = input(&f, 2);
        match index {
            0 => value.dispatch["executable"] = json!("/tmp/agent"),
            1 => value.dispatch["deliveries"][0]["agentBindingId"] = json!("forged"),
            2 => value.user["recipients"][0]["id"] = json!("another"),
            3 => value.turns[0].scope_dir = "../escape".into(),
            4 => value.turns[0].target_id = "removed".into(),
            5 => value.turns[0].context = Some("unadmitted rich context".into()),
            _ => {
                value.user["id"] = json!("delivery:delivery-0:completed");
                value.dispatch["userEntryId"] = value.user["id"].clone();
            }
        }
        assert!(
            client.submit_participant_message(value.clone()).is_err(),
            "{index}"
        );
        assert!(
            client
                .participant_message(value.acceptance_id)
                .unwrap()
                .is_none(),
            "{index}"
        );
    }
    assert!(!f.commands().iter().any(|v| v["type"] == "prompt"));
    drop(client);
    shutdown(runtime).await;
}
