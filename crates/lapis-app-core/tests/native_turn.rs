#[path = "support/native_turn/mod.rs"]
mod support;
use lapis_app_core::services::Runtime;
use lapis_app_protocol::turns::TurnPhase;
use std::{
    fs,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use support::Fixture;

async fn shutdown(runtime: Arc<Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepted_acp_turn_completes_after_client_detach_and_reopens_without_dispatch() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let mut input = f.input();
    input.options.model = Some("fixture-model".into());
    input.options.effort = Some("low".into());
    let accepted = client.submit_turn(input.clone()).unwrap();
    drop(client);
    f.prompted().await;
    assert_eq!(runtime.get(f.vault).unwrap().status().unwrap().clients, 0);
    let completed = f.finished(&runtime, input.request_id).await;
    assert_eq!(completed.phase, TurnPhase::Completed);
    assert_eq!(completed.output.as_deref(), Some("reply café 📝"));
    assert_eq!(accepted.conversation_id, completed.conversation_id);
    let transcript = fs::read_to_string(f.conversation(&completed, "transcript.jsonl")).unwrap();
    let entries: Vec<serde_json::Value> = transcript
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["role"], "user");
    assert_eq!(entries[0]["text"], input.prompt);
    assert_eq!(entries[1]["role"], "assistant");
    assert_eq!(entries[1]["text"], "reply café 📝");
    assert!(!transcript.contains("Separate context"));
    let commands = f.commands();
    let configuration = commands
        .iter()
        .find(|value| value["type"] == "configure")
        .unwrap();
    assert_eq!(configuration["model"], "fixture-model");
    assert_eq!(configuration["effort"], "low");
    assert_eq!(configuration["mode"], "ask");
    let prompt = commands
        .iter()
        .find(|value| value["type"] == "prompt")
        .unwrap();
    assert_eq!(prompt["text"], input.prompt);
    assert_eq!(prompt["context"], "Separate context");
    for name in ["metadata.yaml", "agents.jsonl"] {
        assert!(f.conversation(&completed, name).is_file());
    }
    shutdown(runtime.clone()).await;
    drop(runtime);
    let reopened = f.runtime();
    let client = reopened.get(f.vault).unwrap().attach().unwrap();
    assert_eq!(
        client.turn(input.request_id).unwrap(),
        Some(completed.clone())
    );
    assert_eq!(client.submit_turn(input).unwrap(), completed);
    assert_eq!(f.policy.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        f.commands()
            .iter()
            .filter(|value| value["type"] == "prompt")
            .count(),
        1
    );
    drop(client);
    shutdown(reopened).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn duplicates_freeze_execution_options_and_do_not_claim_another_worker() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let input = f.input();
    let accepted = client.submit_turn(input.clone()).unwrap();
    assert_eq!(
        client.submit_turn(input.clone()).unwrap().conversation_id,
        accepted.conversation_id
    );
    let mut changed = input.clone();
    changed.options.mode = "other".into();
    assert!(client.submit_turn(changed).is_err());
    assert!(client.submit_turn(f.input()).is_err());
    assert_eq!(
        f.finished(&runtime, input.request_id).await.phase,
        TurnPhase::Completed
    );
    assert_eq!(f.policy.calls.load(Ordering::SeqCst), 1);
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unsupported_mode_never_submits_the_authored_prompt() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let mut input = f.input();
    input.options.mode = "denied".into();
    client.submit_turn(input.clone()).unwrap();
    let result = f.finished(&runtime, input.request_id).await;
    assert_eq!(
        result.phase,
        TurnPhase::Failed,
        "{result:?}; commands: {:?}",
        f.commands()
    );
    assert!(!f.commands().iter().any(|value| value["type"] == "prompt"));
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn closed_vault_releases_its_journal_writer_even_while_a_stale_client_survives() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let stale = runtime.get(f.vault).unwrap().attach().unwrap();
    let input = f.input();
    stale.submit_turn(input.clone()).unwrap();
    let completed = f.finished(&runtime, input.request_id).await;
    assert_eq!(completed.phase, TurnPhase::Completed);
    let closing = runtime.clone();
    let vault_id = f.vault;
    tokio::task::spawn_blocking(move || closing.close_vault(vault_id))
        .await
        .unwrap()
        .unwrap();
    assert!(stale.turn(input.request_id).is_err());
    let admitted = runtime.admit(f.vault, &f.root).unwrap();
    let current = admitted.attach().unwrap();
    assert_eq!(current.turn(input.request_id).unwrap(), Some(completed));
    assert_eq!(f.policy.calls.load(Ordering::SeqCst), 1);
    drop(current);
    drop(stale);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_transcript_edits_survive_a_failed_native_publication() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let input = f.input();
    let record = client.submit_turn(input.clone()).unwrap();
    f.prompted().await;
    let path = f.conversation(&record, "transcript.jsonl");
    fs::write(&path, "external preserved text").unwrap();
    let failed = f.finished(&runtime, input.request_id).await;
    assert_eq!(failed.phase, TurnPhase::Unknown);
    assert_eq!(fs::read_to_string(path).unwrap(), "external preserved text");
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_vault_close_waits_for_process_cleanup_without_replaying_the_turn() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let input = f.input();
    client.submit_turn(input.clone()).unwrap();
    f.prompted().await;
    drop(client);
    let closing = runtime.clone();
    tokio::time::timeout(
        Duration::from_secs(5),
        tokio::task::spawn_blocking(move || closing.shutdown()),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    for frame in f.commands() {
        if let Some(pid) = frame["pid"].as_u64() {
            assert!(
                !std::process::Command::new("/bin/kill")
                    .args(["-0", &pid.to_string()])
                    .stderr(std::process::Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            );
        }
    }
    drop(runtime);
    let reopened = f.runtime();
    let record = f.finished(&reopened, input.request_id).await;
    assert_eq!(record.phase, TurnPhase::Unknown);
    assert_eq!(f.policy.calls.load(Ordering::SeqCst), 1);
    shutdown(reopened).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_owner_acknowledges_durable_output_without_a_client_event_stream() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let mut input = f.input();
    input.prompt = "streaming-chunks".into();
    client.submit_turn(input.clone()).unwrap();
    drop(client);
    let completed = f.finished(&runtime, input.request_id).await;
    assert_eq!(completed.phase, TurnPhase::Completed, "{completed:?}");
    assert_eq!(completed.output, Some("x".repeat(400)));
    let text = fs::read_to_string(f.conversation(&completed, "transcript.jsonl")).unwrap();
    let assistant: serde_json::Value = serde_json::from_str(text.lines().nth(1).unwrap()).unwrap();
    assert_eq!(assistant["text"], "x".repeat(400));
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dirty_conversation_editor_blocks_native_replacement_and_survives_detach() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let input = f.input();
    let record = client.submit_turn(input.clone()).unwrap();
    f.prompted().await;
    let path = format!(
        ".lapis/agents/sessions/{}/transcript.jsonl",
        record.conversation_id
    );
    let before = client.read_text(&path).unwrap();
    let editor = client.open_editor(&path).unwrap();
    client.mark_dirty(editor, 1).unwrap();
    drop(client);
    assert_eq!(
        f.finished(&runtime, input.request_id).await.phase,
        TurnPhase::Unknown
    );
    let later = runtime.get(f.vault).unwrap().attach().unwrap();
    assert_eq!(later.read_text(&path).unwrap().text, before.text);
    assert!(runtime.get(f.vault).unwrap().close().is_err());
    later.recover_editor(editor).unwrap();
    later.discard_editor(editor).unwrap();
    drop(later);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stalled_required_capture_keeps_coverage_loss_unknown_and_never_replays() {
    let f = Fixture::new();
    f.policy.stalled_capture.store(true, Ordering::SeqCst);
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let mut input = f.input();
    input.prompt = "burst-chunks".into();
    client.submit_turn(input.clone()).unwrap();
    drop(client);
    let unknown = f.finished(&runtime, input.request_id).await;
    assert_eq!(unknown.phase, TurnPhase::Unknown, "{unknown:?}");
    assert_eq!(
        unknown.failure_code.as_deref(),
        Some("output_coverage_lost")
    );
    let text = fs::read_to_string(f.conversation(&unknown, "transcript.jsonl")).unwrap();
    let terminal: serde_json::Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
    assert_eq!(terminal["type"], "error");
    assert_eq!(terminal["message"], "output_coverage_lost");
    shutdown(runtime.clone()).await;
    drop(runtime);
    let reopened = f.runtime();
    let later = reopened.get(f.vault).unwrap().attach().unwrap();
    assert_eq!(later.submit_turn(input).unwrap(), unknown);
    assert_eq!(f.policy.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        f.commands()
            .iter()
            .filter(|frame| frame["type"] == "prompt")
            .count(),
        1
    );
    drop(later);
    shutdown(reopened).await;
}

struct UnknownStartup;
impl lapis_ai_service::ExecutionBackend for UnknownStartup {
    fn start(
        &self,
        _: lapis_ai_service::runtime::PreparedLaunch,
        _: lapis_ai_service::runtime::SessionOptions,
        _: tokio::sync::watch::Receiver<bool>,
        _: Option<Box<dyn lapis_ai_service::runtime::SessionProcessor>>,
    ) -> lapis_ai_service::ExecutionFuture<'_, Arc<dyn lapis_ai_service::ExecutionSession>> {
        Box::pin(async { Err(lapis_ai_service::runtime::Error("startup_cleanup_unknown")) })
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn uncertain_startup_retains_work_and_reports_close_failure_without_hanging() {
    let f = Fixture::new();
    let storage = lapis_app_core::services::TurnStorage::new(
        &f.profile,
        f.policy.clone(),
        tokio::runtime::Handle::current(),
    )
    .unwrap()
    .with_backend(Arc::new(UnknownStartup));
    let runtime = Arc::new(Runtime::default().with_turns(storage));
    let vault = runtime.admit(f.vault, &f.root).unwrap();
    let client = vault.attach().unwrap();
    let input = f.input();
    client.submit_turn(input.clone()).unwrap();
    let unknown = f.finished(&runtime, input.request_id).await;
    assert_eq!(unknown.phase, TurnPhase::Unknown);
    assert_eq!(unknown.failure_code.as_deref(), Some("cleanup_unknown"));
    assert!(vault.status().unwrap().work > 0);
    for enabled in [false, true] {
        assert_eq!(
            client.set_turns_enabled(enabled).unwrap_err().code(),
            lapis_app_protocol::ErrorCode::Io
        );
    }
    assert_eq!(
        client.submit_turn(f.input()).unwrap_err().code(),
        lapis_app_protocol::ErrorCode::Unsupported
    );
    assert_eq!(client.submit_turn(input).unwrap(), unknown);
    drop(client);
    let closing = vault.clone();
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        tokio::task::spawn_blocking(move || closing.close()),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(result.is_err());
    assert!(vault.status().unwrap().work > 0);
    assert_eq!(f.policy.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn accepted_turn_retains_prepared_native_configuration_without_observer_policy_lookup() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let input = f.input();
    client.submit_turn(input.clone()).unwrap();
    // The current-thread runtime has not yielded to its native worker yet.
    f.policy
        .configuration_unavailable
        .store(true, Ordering::SeqCst);
    drop(client);
    let completed = f.finished(&runtime, input.request_id).await;
    assert_eq!(completed.phase, TurnPhase::Completed);
    assert_eq!(f.policy.calls.load(Ordering::SeqCst), 1);
    shutdown(runtime).await;
}
