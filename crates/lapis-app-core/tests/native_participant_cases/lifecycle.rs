use super::*;
use lapis_app_protocol::turns::{TurnCommand, TurnValue};

#[tokio::test(flavor = "current_thread")]
async fn cancelling_an_unstarted_participant_records_cancelled_without_prompting_the_delivery() {
    let f = Fixture::new();
    peer(&f);
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let input = input(&f, 2);
    client.submit_participant_message(input.clone()).unwrap();
    assert!(matches!(
        client
            .call_turn(TurnCommand::Cancel {
                request_id: input.turns[0].request_id
            })
            .unwrap(),
        TurnValue::CancelRequested { accepted: true }
    ));
    let done = settled(&f, input.acceptance_id).await;
    assert_eq!(done.deliveries[0].state, ParticipantState::Cancelled);
    assert_eq!(done.deliveries[1].state, ParticipantState::Completed);
    let prompts: Vec<_> = f
        .commands()
        .into_iter()
        .filter(|v| v["type"] == "prompt")
        .collect();
    assert_eq!(prompts.len(), 1);
    assert_eq!(
        prompts[0]["requestId"],
        input.turns[1].request_id.to_string()
    );
    let entries = rows(&f, &done, "transcript.jsonl");
    assert!(
        !entries
            .iter()
            .any(|v| v["id"] == format!("delivery:{}:started", done.deliveries[0].delivery_id))
    );
    let cancelled = entries
        .iter()
        .find(|v| v["id"] == format!("delivery:{}:cancelled", done.deliveries[0].delivery_id))
        .unwrap();
    assert!(cancelled["delivery"].get("agentBindingId").is_none());
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disabling_native_participants_cancels_and_drains_every_started_worker() {
    let f = Fixture::new();
    peer(&f);
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let mut input = input(&f, 2);
    for turn in &mut input.turns {
        turn.options.model = Some("held-disable".into());
    }
    client.submit_participant_message(input.clone()).unwrap();
    prompted(&f, 2).await;
    assert!(client.submit_turn(f.input()).is_err());
    let retained = runtime.get(f.vault).unwrap().attach().unwrap();
    assert!(
        !tokio::task::spawn_blocking(move || retained.set_turns_enabled(false))
            .await
            .unwrap()
            .unwrap()
    );
    let done = client
        .participant_message(input.acceptance_id)
        .unwrap()
        .unwrap();
    assert!(
        done.deliveries
            .iter()
            .all(|d| d.state == ParticipantState::Unknown)
    );
    let mut next = input.clone();
    next.acceptance_id = Uuid::new_v4();
    for turn in &mut next.turns {
        turn.request_id = Uuid::new_v4();
    }
    assert!(client.submit_participant_message(next).is_err());
    assert_eq!(
        f.commands()
            .iter()
            .filter(|v| v["type"] == "prompt")
            .count(),
        2
    );
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_one_started_participant_preserves_its_live_sibling_and_independent_outcome() {
    let f = Fixture::new();
    peer(&f);
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let mut input = input(&f, 2);
    for (turn, variant) in input
        .turns
        .iter_mut()
        .zip(["held-cancelled", "held-survivor"])
    {
        turn.options.model = Some(variant.into());
    }
    client.submit_participant_message(input.clone()).unwrap();
    prompted(&f, 2).await;
    assert!(matches!(
        client
            .call_turn(TurnCommand::Cancel {
                request_id: input.turns[0].request_id
            })
            .unwrap(),
        TurnValue::CancelRequested { accepted: true }
    ));
    until(|| stored(&f, input.acceptance_id).deliveries[0].state == ParticipantState::Unknown)
        .await;
    assert_eq!(
        stored(&f, input.acceptance_id).deliveries[1].state,
        ParticipantState::Started
    );
    release(&f, Some("held-survivor"));
    let done = settled(&f, input.acceptance_id).await;
    assert_eq!(done.deliveries[0].state, ParticipantState::Unknown);
    assert_eq!(done.deliveries[1].state, ParticipantState::Completed);
    assert!(done.deliveries.iter().all(|d| d.outcome_published));
    assert!(
        rows(&f, &done, "transcript.jsonl")
            .iter()
            .any(
                |v| v["agentBindingId"] == done.deliveries[1].agent_binding_id.to_string()
                    && v["text"] == "held-survivor:α:β:終"
            )
    );
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn active_participant_vault_close_and_quit_retire_admission_after_all_owned_work_drains() {
    for quit in [false, true] {
        let f = Fixture::new();
        peer(&f);
        let runtime = f.runtime();
        let client = runtime.get(f.vault).unwrap().attach().unwrap();
        let mut input = input(&f, 2);
        for turn in &mut input.turns {
            turn.options.model = Some("held-close".into());
        }
        client.submit_participant_message(input.clone()).unwrap();
        prompted(&f, 2).await;
        let retained = runtime.clone();
        let vault = f.vault;
        tokio::task::spawn_blocking(move || {
            if quit {
                retained.shutdown()
            } else {
                retained.close_vault(vault)
            }
        })
        .await
        .unwrap()
        .unwrap();
        let done = stored(&f, input.acceptance_id);
        assert!(
            done.deliveries
                .iter()
                .all(|d| d.state == ParticipantState::Unknown)
        );
        assert!(client.submit_participant_message(input).is_err());
        assert_eq!(
            f.commands()
                .iter()
                .filter(|v| v["type"] == "prompt")
                .count(),
            2
        );
        drop(client);
        shutdown(runtime).await;
    }
}
