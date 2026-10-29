#[path = "support/native_turn/mod.rs"]
mod support;
use lapis_app_core::services::Client;
use lapis_app_protocol::{
    ErrorCode,
    turns::{TurnCommand, TurnPhase, TurnValue},
};
use serde_json::json;
use std::{fs, sync::Arc, time::Duration};
use support::Fixture;

fn command(enabled: bool) -> TurnCommand {
    serde_json::from_value(json!({"kind":"set_enabled","enabled":enabled})).unwrap()
}
async fn set(client: Arc<Client>, enabled: bool) -> lapis_app_core::services::Result<TurnValue> {
    tokio::task::spawn_blocking(move || client.call_turn(command(enabled)))
        .await
        .unwrap()
}
async fn shutdown(runtime: Arc<lapis_app_core::services::Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disable_drains_a_running_turn_and_enable_never_replays_it() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = Arc::new(runtime.get(f.vault).unwrap().attach().unwrap());
    let input = f.input();
    client.submit_turn(input.clone()).unwrap();
    f.prompted().await;
    let disabled = set(client.clone(), false).await.unwrap();
    assert_eq!(
        serde_json::to_value(disabled).unwrap(),
        json!({"kind":"availability","enabled":false})
    );
    let record = client.turn(input.request_id).unwrap().unwrap();
    assert!(matches!(
        record.phase,
        TurnPhase::Cancelled | TurnPhase::Unknown
    ));
    assert!(
        fs::read_to_string(f.conversation(&record, "transcript.jsonl"))
            .unwrap()
            .contains(&input.prompt)
    );
    assert_eq!(
        client.submit_turn(f.input()).unwrap_err().code(),
        ErrorCode::Unsupported
    );
    assert_eq!(client.submit_turn(input).unwrap(), record);
    set(client.clone(), false).await.unwrap();
    set(client.clone(), true).await.unwrap();
    assert_eq!(
        f.commands()
            .iter()
            .filter(|v| v["type"] == "prompt")
            .count(),
        1
    );
    let fresh = client.submit_turn(f.input()).unwrap();
    assert_eq!(
        f.finished(&runtime, fresh.input.request_id).await.phase,
        TurnPhase::Completed
    );
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_previously_retained_client_cannot_submit_after_the_disable_fence() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = Arc::new(runtime.get(f.vault).unwrap().attach().unwrap());
    let retained = client.clone();
    set(client.clone(), false).await.unwrap();
    assert_eq!(
        retained.submit_turn(f.input()).unwrap_err().code(),
        ErrorCode::Unsupported
    );
    assert!(retained.turns().unwrap().is_empty());
    assert!(f.commands().is_empty());
    set(client.clone(), true).await.unwrap();
    let accepted = retained.submit_turn(f.input()).unwrap();
    f.prompted().await;
    assert_eq!(
        f.finished(&runtime, accepted.input.request_id).await.phase,
        TurnPhase::Completed
    );
    drop(retained);
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn losing_the_disable_waiter_does_not_reopen_admission_or_abandon_cleanup() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = Arc::new(runtime.get(f.vault).unwrap().attach().unwrap());
    let input = f.input();
    client.submit_turn(input.clone()).unwrap();
    f.prompted().await;
    let stopping = client.clone();
    let task = tokio::task::spawn_blocking(move || stopping.call_turn(command(false)));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Err(error) = client.call_turn(TurnCommand::Targets {}) {
                assert_eq!(error.code(), ErrorCode::Unsupported);
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    let completed = f.finished(&runtime, input.request_id).await;
    assert!(matches!(
        completed.phase,
        TurnPhase::Cancelled | TurnPhase::Unknown
    ));
    set(client.clone(), true).await.unwrap();
    let fresh = client.submit_turn(f.input()).unwrap();
    assert_eq!(
        f.finished(&runtime, fresh.input.request_id).await.phase,
        TurnPhase::Completed
    );
    drop(client);
    shutdown(runtime).await;
}
