use jj_vault::Phase;
use lapis_app_core::services::{BackupStorage, Client, Runtime};
use lapis_app_protocol::editing_intents::EditingIntentCommand;
use lapis_app_protocol::{ErrorCode, ExpectedVersion, repository_backup::*};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, sync::Arc, time::Duration};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saved_false_selection_is_honored_before_initial_capture_without_an_app() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("vault");
    let history = temp.path().join("history");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&history).unwrap();
    fs::create_dir(root.join(".lapis")).unwrap();
    let configuration = r#"{"keep":"original","pluginData":{"repository-history":{"automaticBackup":false,"other":"retained"},"other":{"value":4}}}"#;
    fs::write(root.join(".lapis/app.json"), configuration).unwrap();
    fs::write(root.join("Note.md"), "saved while disabled").unwrap();
    let id = Uuid::new_v4();
    for _ in 0..2 {
        let runtime = Runtime::default().with_backup(BackupStorage::new(&history).unwrap());
        let client = runtime.admit(id, &root).unwrap().attach().unwrap();
        let status = client.start_backup().await.unwrap();
        assert_eq!(status.borrow().phase, Phase::Closed);
        assert!(!history.join(id.to_string()).exists());
        assert!(!root.join(".jj").exists());
        tokio::task::spawn_blocking(move || runtime.shutdown())
            .await
            .unwrap()
            .unwrap();
    }
    assert_eq!(
        fs::read_to_string(root.join(".lapis/app.json")).unwrap(),
        configuration
    );
    assert_eq!(
        fs::read_to_string(root.join("Note.md")).unwrap(),
        "saved while disabled"
    );
}

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    history: PathBuf,
    runtime: Arc<Runtime>,
    client: Arc<Client>,
}
fn fixture(value: Option<Value>) -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let root = base.join("notes");
    let history = base.join("history");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&history).unwrap();
    fs::write(root.join("Note.md"), "initial").unwrap();
    if let Some(value) = value {
        fs::create_dir(root.join(".lapis")).unwrap();
        fs::write(root.join(".lapis/app.json"), value.to_string()).unwrap();
    }
    let runtime = Arc::new(Runtime::default().with_backup(BackupStorage::new(&history).unwrap()));
    let client = Arc::new(
        runtime
            .admit(Uuid::new_v4(), &root)
            .unwrap()
            .attach()
            .unwrap(),
    );
    Fixture {
        _temp: temp,
        root,
        history,
        runtime,
        client,
    }
}
async fn status(f: &Fixture) -> RepositoryBackupSnapshot {
    f.client
        .control_repository_backup(RepositoryBackupCommand::Status {})
        .await
        .unwrap()
}
async fn select(
    f: &Fixture,
    enabled: bool,
    expected: ExpectedVersion,
) -> lapis_app_core::services::Result<RepositoryBackupSnapshot> {
    f.client
        .control_repository_backup(RepositoryBackupCommand::SetEnabled { enabled, expected })
        .await
}
async fn close(runtime: Arc<Runtime>) {
    tokio::time::timeout(
        Duration::from_secs(10),
        tokio::task::spawn_blocking(move || runtime.shutdown()),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn passive_status_never_materializes_configuration_or_initializes_history() {
    let f = fixture(None);
    for _ in 0..2 {
        let value = status(&f).await;
        assert!(value.enabled);
        assert!(matches!(value.configuration, ExpectedVersion::Missing {}));
        assert!(matches!(value.status.phase, RepositoryBackupPhase::Failed));
    }
    assert!(!f.root.join(".lapis").exists());
    assert_eq!(fs::read_dir(&f.history).unwrap().count(), 0);
    close(f.runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_selection_preserves_other_settings_and_survives_detach_and_cold_boot() {
    let original = json!({"keep":"retained","pluginData":{"other":{"value":2},"repository-history":{"automaticBackup":false,"showWorkingCopyStatus":false}}});
    let f = fixture(Some(original.clone()));
    let first = status(&f).await;
    assert!(!first.enabled);
    let enabled = select(&f, true, first.configuration).await.unwrap();
    assert!(enabled.enabled);
    assert!(matches!(enabled.status.phase, RepositoryBackupPhase::Ready));
    assert!(enabled.status.checkpoint.is_some());
    fs::write(f.root.join("Note.md"), "manually captured").unwrap();
    let captured = f
        .client
        .control_repository_backup(RepositoryBackupCommand::Capture {})
        .await
        .unwrap();
    assert!(matches!(
        captured.status.phase,
        RepositoryBackupPhase::Ready
    ));
    let disabled = select(&f, false, captured.configuration).await.unwrap();
    assert!(!disabled.enabled);
    assert!(matches!(
        disabled.status.phase,
        RepositoryBackupPhase::Closed
    ));
    assert_eq!(
        serde_json::from_str::<Value>(&fs::read_to_string(f.root.join(".lapis/app.json")).unwrap())
            .unwrap(),
        original
    );
    let id = f.client.descriptor().vault_id;
    f.client.detach().unwrap();
    let later = f.runtime.get(id).unwrap().attach().unwrap();
    assert_eq!(
        later.start_backup().await.unwrap().borrow().phase,
        Phase::Closed
    );
    close(f.runtime).await;
    let runtime = Arc::new(Runtime::default().with_backup(BackupStorage::new(&f.history).unwrap()));
    let cold = runtime.admit(id, &f.root).unwrap().attach().unwrap();
    assert_eq!(
        cold.start_backup().await.unwrap().borrow().phase,
        Phase::Closed
    );
    assert!(matches!(
        cold.control_repository_backup(RepositoryBackupCommand::Capture {})
            .await
            .unwrap_err()
            .0
            .code,
        ErrorCode::Unsupported
    ));
    close(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_version_and_dirty_configuration_refuse_selection_without_side_effects() {
    let f = fixture(Some(
        json!({"pluginData":{"repository-history":{"automaticBackup":false}}}),
    ));
    let first = status(&f).await;
    let path = f.root.join(".lapis/app.json");
    let newer =
        r#"{"other":"newer","pluginData":{"repository-history":{"automaticBackup":false}}}"#;
    fs::write(&path, newer).unwrap();
    assert_eq!(
        select(&f, true, first.configuration)
            .await
            .unwrap_err()
            .0
            .code,
        ErrorCode::Conflict
    );
    let current = status(&f).await;
    let editor = f.client.open_editor(".lapis/app.json").unwrap();
    f.client.mark_dirty(editor, 1).unwrap();
    assert_eq!(
        select(&f, true, current.configuration)
            .await
            .unwrap_err()
            .0
            .code,
        ErrorCode::Busy
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), newer);
    assert_eq!(fs::read_dir(&f.history).unwrap().count(), 0);
    f.client.discard_editor(editor).unwrap();
    close(f.runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_saved_configuration_never_initializes_or_overwrites() {
    for value in [
        Value::Null,
        json!({"pluginData":[]}),
        json!({"pluginData":{"repository-history":false}}),
    ] {
        let f = fixture(Some(value.clone()));
        assert!(f.client.start_backup().await.is_err());
        assert!(
            f.client
                .control_repository_backup(RepositoryBackupCommand::Status {})
                .await
                .is_err()
        );
        assert_eq!(
            fs::read_to_string(f.root.join(".lapis/app.json")).unwrap(),
            value.to_string()
        );
        assert_eq!(fs::read_dir(&f.history).unwrap().count(), 0);
        close(f.runtime).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lost_selection_waiter_and_passive_status_wait_for_the_accepted_drain() {
    let f = fixture(None);
    f.client.start_backup().await.unwrap();
    let expected = status(&f).await.configuration;
    let (entered, ready) = tokio::sync::oneshot::channel();
    let (release, wait) = tokio::sync::oneshot::channel();
    let client = f.client.clone();
    let holding = tokio::spawn(async move {
        client
            .with_backup(move |_| {
                Box::pin(async move {
                    entered.send(()).unwrap();
                    wait.await.unwrap();
                    Ok(())
                })
            })
            .await
            .unwrap();
    });
    ready.await.unwrap();
    let client = f.client.clone();
    let selecting = tokio::spawn(async move {
        client
            .control_repository_backup(RepositoryBackupCommand::SetEnabled {
                enabled: false,
                expected,
            })
            .await
    });
    let vault = f.runtime.get(f.client.descriptor().vault_id).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while vault.status().unwrap().work < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    selecting.abort();
    let _ = selecting.await;
    let client = f.client.clone();
    let observing = tokio::spawn(async move {
        client
            .control_repository_backup(RepositoryBackupCommand::Status {})
            .await
            .unwrap()
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while vault.status().unwrap().work < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let premature = observing.is_finished();
    release.send(()).unwrap();
    holding.await.unwrap();
    let final_status = observing.await.unwrap();
    assert!(!premature);
    assert!(!final_status.enabled);
    assert!(matches!(
        final_status.status.phase,
        RepositoryBackupPhase::Closed
    ));
    close(f.runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clean_configuration_intent_must_yield_before_native_selection() {
    let f = fixture(Some(
        json!({"pluginData":{"repository-history":{"automaticBackup":false}}}),
    ));
    let expected = status(&f).await.configuration;
    let editor_id = Uuid::new_v4();
    f.client
        .editing_intents(EditingIntentCommand::Acquire {
            editor_id,
            path: ".lapis/app.json".into(),
        })
        .unwrap();
    assert_eq!(
        select(&f, true, expected.clone()).await.unwrap_err().code(),
        ErrorCode::Busy
    );
    assert_eq!(fs::read_dir(&f.history).unwrap().count(), 0);
    f.client
        .editing_intents(EditingIntentCommand::Yield {
            editor_id,
            generation: 1,
            revision: 0,
            expected: expected.clone(),
        })
        .unwrap();
    assert!(select(&f, true, expected).await.unwrap().enabled);
    close(f.runtime).await;
}
