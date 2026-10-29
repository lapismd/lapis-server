#[path = "support/native_turn/mod.rs"]
mod support;
use lapis_app_protocol::turns::{TurnContinuation, TurnPhase, TurnRecord};
use serde_json::Value;
use std::{fs, sync::Arc};
use support::Fixture;

fn next(fixture: &Fixture, parent: &TurnRecord) -> lapis_app_protocol::turns::TurnInput {
    let mut input = fixture.input();
    input.prompt = "A distinct follow-up".into();
    input.continuation = Some(TurnContinuation {
        conversation_id: parent.conversation_id,
        after_request_id: parent.input.request_id,
    });
    input
}
async fn shutdown(runtime: Arc<lapis_app_core::services::Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_followups_preserve_one_transcript_after_detach_and_cold_owner_recovery() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let accepted = client.submit_turn(f.input()).unwrap();
    f.prompted().await;
    let first = f.finished(&runtime, accepted.input.request_id).await;
    assert_eq!(first.phase, TurnPhase::Completed);
    let path = f.conversation(&first, "transcript.jsonl");
    let original = fs::read_to_string(&path).unwrap();
    let metadata_path = f.conversation(&first, "metadata.yaml");
    let mut metadata: Value = serde_json::from_slice(&fs::read(&metadata_path).unwrap()).unwrap();
    metadata["title"] = Value::String("A retained user title".into());
    fs::write(&metadata_path, serde_json::to_vec(&metadata).unwrap()).unwrap();
    let agents_path = f.conversation(&first, "agents.jsonl");
    let original_agents = fs::read_to_string(&agents_path).unwrap();
    let followup = next(&f, &first);
    client.submit_turn(followup.clone()).unwrap();
    drop(client);
    let second = f.finished(&runtime, followup.request_id).await;
    assert_eq!(second.phase, TurnPhase::Completed);
    assert_eq!(second.conversation_id, first.conversation_id);
    let transcript = fs::read_to_string(&path).unwrap();
    assert!(transcript.starts_with(&original));
    let updated: Value = serde_json::from_slice(&fs::read(metadata_path).unwrap()).unwrap();
    assert_eq!(updated["title"], metadata["title"]);
    assert_eq!(updated["createdAt"], metadata["createdAt"]);
    assert!(
        fs::read_to_string(agents_path)
            .unwrap()
            .starts_with(&original_agents)
    );
    let entries: Vec<Value> = transcript
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(entries.len(), 4);
    assert_eq!(entries[2]["text"], followup.prompt);
    assert_eq!(entries[2]["role"], "user");
    assert_eq!(entries[3]["role"], "assistant");
    let prompts: Vec<Value> = f
        .commands()
        .into_iter()
        .filter(|v| v["type"] == "prompt")
        .collect();
    assert_eq!(prompts.len(), 2);
    assert_eq!(prompts[1]["text"], followup.prompt);
    let context = prompts[1]["context"].as_str().unwrap();
    assert!(context.contains(&first.input.prompt));
    assert!(context.contains("reply café 📝"));
    assert!(context.contains("Separate context"));
    assert!(!context.contains(&followup.prompt));
    shutdown(runtime).await;
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    assert_eq!(client.turn(followup.request_id).unwrap().unwrap(), second);
    assert_eq!(client.submit_turn(followup).unwrap(), second);
    let third = client.submit_turn(next(&f, &second)).unwrap();
    let third = f.finished(&runtime, third.input.request_id).await;
    assert_eq!(third.phase, TurnPhase::Completed);
    assert_eq!(third.conversation_id, first.conversation_id);
    assert!(fs::read_to_string(path).unwrap().starts_with(&transcript));
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
async fn unreadable_history_refuses_before_acceptance_and_preserves_the_file() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let accepted = client.submit_turn(f.input()).unwrap();
    f.prompted().await;
    let first = f.finished(&runtime, accepted.input.request_id).await;
    let path = f.conversation(&first, "transcript.jsonl");
    let original = fs::read_to_string(&path).unwrap();
    fs::write(&path, "unreadable external content\n").unwrap();
    let followup = next(&f, &first);
    assert!(client.submit_turn(followup.clone()).is_err());
    assert!(client.turn(followup.request_id).unwrap().is_none());
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "unreadable external content\n"
    );
    fs::write(&path, original).unwrap();
    let accepted = client.submit_turn(followup).unwrap();
    assert_eq!(
        f.finished(&runtime, accepted.input.request_id).await.phase,
        TurnPhase::Completed
    );
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bounded_unicode_history_preserves_full_files_and_the_latest_response() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let mut input = f.input();
    input.prompt = "📝".repeat(80_000);
    let accepted = client.submit_turn(input).unwrap();
    f.prompted().await;
    let first = f.finished(&runtime, accepted.input.request_id).await;
    let path = f.conversation(&first, "transcript.jsonl");
    let original = fs::read_to_string(&path).unwrap();
    let accepted = client.submit_turn(next(&f, &first)).unwrap();
    assert_eq!(
        f.finished(&runtime, accepted.input.request_id).await.phase,
        TurnPhase::Completed
    );
    let prompts: Vec<Value> = f
        .commands()
        .into_iter()
        .filter(|v| v["type"] == "prompt")
        .collect();
    let context = prompts[1]["context"].as_str().unwrap();
    assert!(context.contains("middle omitted by Lapis"));
    assert!(context.contains("assistant: \"reply café 📝\""));
    assert!(context.chars().count() < 48_500);
    assert!(fs::read_to_string(path).unwrap().starts_with(&original));
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "current_thread")]
async fn accepted_followup_pins_history_and_refuses_an_external_change_before_dispatch() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let accepted = client.submit_turn(f.input()).unwrap();
    f.prompted().await;
    let first = f.finished(&runtime, accepted.input.request_id).await;
    let path = f.conversation(&first, "transcript.jsonl");
    let accepted = client.submit_turn(next(&f, &first)).unwrap();
    // This thread cannot run the newly spawned task until the next await.
    fs::write(&path, "An external edit after acceptance\n").unwrap();
    let failed = f.finished(&runtime, accepted.input.request_id).await;
    assert_eq!(failed.phase, TurnPhase::Failed);
    assert_eq!(
        failed.failure_code.as_deref(),
        Some("conversation_preparation_failed")
    );
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "An external edit after acceptance\n"
    );
    assert_eq!(
        f.commands()
            .iter()
            .filter(|v| v["type"] == "prompt")
            .count(),
        1
    );
    drop(client);
    shutdown(runtime).await;
}
