#[path = "support/native_turn/mod.rs"]
mod support;
use lapis_app_core::services::{ParticipantMessageInput, ParticipantState};
use lapis_app_protocol::turns::{TurnPhase, TurnRecord};
use serde_json::{Value, json};
use std::{fs, time::Duration};
use support::Fixture;
use uuid::Uuid;

fn input(f: &Fixture) -> ParticipantMessageInput {
    let recipients = json!([
        {"id":"writer","token":"jj-store-agent:777269746572;","label":"Captured Writer"},
        {"id":"reviewer","token":"jj-store-agent:7265766965776572;","label":"Captured Reviewer"}
    ]);
    let user = json!({"schemaVersion":3,"id":"user-authored-café","type":"message","role":"user","text":"Authored café 📝","createdAt":"2026-10-27T04:30:00.000Z","recipients":recipients});
    ParticipantMessageInput {
        profiles: None,
        acceptance_id: Uuid::new_v4(),
        user,
        dispatch: json!({"version":1,"id":"dispatch-authored","userEntryId":"user-authored-café","startedAt":"2026-10-27T04:30:00.000Z","deliveries":[
            {"id":"delivery:writer","recipient":recipients[0],"state":"pending"},
            {"id":"delivery:reviewer","recipient":recipients[1],"state":"pending"}
        ]}),
        turns: (0..2)
            .map(|_| {
                let mut t = f.input();
                t.context = None;
                t
            })
            .collect(),
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_native_participant_owner_retains_authored_parent_and_independent_outputs_after_detach()
{
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let input = input(&f);
    let accepted = client.submit_participant_message(input.clone()).unwrap();
    assert_eq!(accepted.input, input);
    drop(client);
    f.prompted().await;
    let records = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let records: Option<Vec<TurnRecord>> = input
                .turns
                .iter()
                .map(|t| {
                    fs::read(
                        f.profile
                            .join(f.vault.to_string())
                            .join(format!("{}.json", t.request_id)),
                    )
                    .ok()
                    .and_then(|s| serde_json::from_slice(&s).ok())
                })
                .collect();
            if let Some(records) = records
                && records
                    .iter()
                    .all(|r| !matches!(r.phase, TurnPhase::Accepted | TurnPhase::Running))
            {
                break records;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        records.iter().all(|r| r.phase == TurnPhase::Completed),
        "{records:?}"
    );
    assert_eq!(
        f.finished(&runtime, input.turns[0].request_id).await,
        records[0]
    );
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let retained = client
        .participant_message(input.acceptance_id)
        .unwrap()
        .unwrap();
    assert_eq!(retained.input, input);
    assert!(
        retained
            .deliveries
            .iter()
            .all(|d| d.state == ParticipantState::Completed)
    );
    assert_ne!(
        retained.deliveries[0].agent_binding_id,
        retained.deliveries[1].agent_binding_id
    );
    let read = |name| {
        fs::read_to_string(f.conversation(&records[0], name))
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str::<Value>(s).unwrap())
            .collect::<Vec<_>>()
    };
    let transcript = read("transcript.jsonl");
    let users: Vec<_> = transcript
        .iter()
        .filter(|v| v["type"] == "message" && v["role"] == "user")
        .collect();
    assert_eq!(users, [&input.user]);
    let assistants: Vec<_> = transcript
        .iter()
        .filter(|v| v["type"] == "message" && v["role"] == "assistant")
        .collect();
    assert_eq!(assistants.len(), 2);
    let bindings = read("agents.jsonl");
    for (index, delivery) in retained.deliveries.iter().enumerate() {
        let binding = delivery.agent_binding_id.to_string();
        assert!(
            bindings
                .iter()
                .any(|v| v["id"] == binding && v["participant"] == input.user["recipients"][index])
        );
        assert!(assistants.iter().any(|v| v["agentBindingId"] == binding
            && v["parentId"] == input.user["id"]
            && v["text"] == "reply café 📝"));
        for state in ["started", "completed"] {
            assert!(transcript.iter().any(|v| v["id"]
                == format!("delivery:{}:{state}", delivery.delivery_id)
                && v["dispatchId"] == input.dispatch["id"]
                && v["parentId"] == input.user["id"]
                && v["delivery"]["agentBindingId"] == binding
                && v["delivery"]["state"] == state));
        }
    }
    assert_eq!(
        f.commands()
            .iter()
            .filter(|v| v["type"] == "prompt")
            .count(),
        2
    );
    drop(client);
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
