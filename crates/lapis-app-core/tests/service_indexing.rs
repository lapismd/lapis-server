use lapis_app_core::{
    embedding::host::NativeEmbeddingHost,
    services::{DatabaseStorage, Runtime},
};
use lapis_app_protocol::{
    VaultPhase,
    database::call::{DatabaseCall, DatabaseMethod, DatabaseValue},
};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

fn runtime(profile: &tempfile::TempDir) -> Arc<Runtime> {
    Arc::new(Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    ))
}
async fn row(vault: &Arc<lapis_app_core::services::VaultService>, path: &str) -> Value {
    let client = vault.attach().unwrap();
    match client
        .call_database(DatabaseCall {
            method: DatabaseMethod::GetIndexedFile,
            args: vec![DatabaseValue::Value { value: json!(path) }],
        })
        .await
        .unwrap()
    {
        DatabaseValue::Value { value } => value,
        DatabaseValue::Undefined {} => Value::Null,
    }
}
async fn until(mut ready: impl AsyncFnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready().await {
        assert!(
            Instant::now() < deadline,
            "native reconciliation did not complete"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
async fn close(runtime: Arc<Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn real_markdown_indexes_on_startup_without_a_client_scheduler() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("Note.md"),
        "---\ntitle: Café 📝\ntags: [team/notes]\n---\n# Heading 📝\n[[Target]] #work\n",
    )
    .unwrap();
    std::fs::write(root.path().join("Target.md"), "# Target\n").unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_markdown_indexing().await.unwrap();
    until(async || vault.indexing_status().unwrap().indexed >= 2).await;
    let note = row(&vault, "Note.md").await;
    assert_eq!(
        note["metadata"]["metadata"]["headings"][0]["heading"],
        "Heading 📝"
    );
    assert!(
        note["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["name"] == "title" && p["value"] == "Café 📝")
    );
    assert_eq!(note["links"][0]["resolvedTargetPath"], "Target.md");
    assert!(
        note["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["hierarchy"] == json!(["team", "team/notes"]))
    );
    assert_eq!(vault.status().unwrap().clients, 0);
    close(runtime).await;
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
}

#[tokio::test]
async fn external_create_edit_and_delete_reconcile_after_the_last_client_detaches() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Note.md"), "# Before\n").unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    vault.start_markdown_indexing().await.unwrap();
    until(async || vault.indexing_status().unwrap().indexed >= 1).await;
    drop(client);
    std::fs::write(root.path().join("Note.md"), "# After 📝\n").unwrap();
    std::fs::write(root.path().join("Added.md"), "# New\n").unwrap();
    until(async || vault.indexing_status().unwrap().indexed >= 3).await;
    assert_eq!(
        row(&vault, "Note.md").await["metadata"]["metadata"]["headings"][0]["heading"],
        "After 📝"
    );
    assert!(!row(&vault, "Added.md").await.is_null());
    std::fs::remove_file(root.path().join("Note.md")).unwrap();
    until(async || vault.indexing_status().unwrap().deleted >= 1).await;
    assert!(row(&vault, "Note.md").await.is_null());
    assert_eq!(vault.status().unwrap().clients, 0);
    close(runtime).await;
}

#[tokio::test]
async fn one_owner_skips_unchanged_records_and_ignores_hidden_files_and_symlinks() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Note.md"), "# Visible\n").unwrap();
    std::fs::create_dir(root.path().join(".lapis")).unwrap();
    std::fs::write(root.path().join(".lapis/Hidden.md"), "# Hidden\n").unwrap();
    std::fs::write(outside.path().join("Secret.md"), "# Secret\n").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("Secret.md"),
        root.path().join("Escape.md"),
    )
    .unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let (a, b) = tokio::join!(
        vault.start_markdown_indexing(),
        vault.start_markdown_indexing()
    );
    a.unwrap();
    b.unwrap();
    until(async || vault.indexing_status().unwrap().indexed == 1).await;
    let passes = vault.indexing_status().unwrap().passes;
    std::fs::write(root.path().join("signal.txt"), "rescan").unwrap();
    until(async || vault.indexing_status().unwrap().passes > passes).await;
    assert_eq!(vault.indexing_status().unwrap().indexed, 1);
    assert!(row(&vault, ".lapis/Hidden.md").await.is_null());
    assert!(row(&vault, "Escape.md").await.is_null());
    close(runtime).await;
}

#[tokio::test]
async fn source_hash_detects_changes_with_identical_mtime_and_length() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Note.md");
    std::fs::write(&path, "# Before\n").unwrap();
    let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_markdown_indexing().await.unwrap();
    until(async || vault.indexing_status().unwrap().indexed == 1).await;
    std::fs::write(&path, "# After!\n").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(mtime))
        .unwrap();
    until(async || {
        row(&vault, "Note.md").await["metadata"]["metadata"]["headings"][0]["heading"] == "After!"
    })
    .await;
    close(runtime).await;
}

#[tokio::test]
async fn cold_reopen_repairs_changes_made_with_the_service_stopped() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Note.md"), "# Before\n").unwrap();
    let id = Uuid::new_v4();
    let first = runtime(&profile);
    let old = first.admit(id, root.path()).unwrap();
    old.start_markdown_indexing().await.unwrap();
    until(async || old.indexing_status().unwrap().indexed == 1).await;
    close(first).await;
    std::fs::write(root.path().join("Note.md"), "# Cold recovery\n").unwrap();
    let next = runtime(&profile);
    let vault = next.admit(id, root.path()).unwrap();
    vault.start_markdown_indexing().await.unwrap();
    until(async || vault.indexing_status().unwrap().indexed == 1).await;
    assert_eq!(
        row(&vault, "Note.md").await["metadata"]["metadata"]["headings"][0]["heading"],
        "Cold recovery"
    );
    close(next).await;
}

#[tokio::test]
async fn dirty_close_refusal_keeps_the_unattended_owner_running() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Note.md"), "# Before\n").unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_markdown_indexing().await.unwrap();
    until(async || vault.indexing_status().unwrap().indexed == 1).await;
    let client = vault.attach().unwrap();
    let editor = client.open_editor("Note.md").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    assert_eq!(
        vault.close().unwrap_err().code(),
        lapis_app_protocol::ErrorCode::Busy
    );
    std::fs::write(root.path().join("Other.md"), "# Still running\n").unwrap();
    until(async || vault.indexing_status().unwrap().indexed >= 2).await;
    client.discard_editor(editor).unwrap();
    close(runtime).await;
}

#[tokio::test]
async fn parser_failures_are_observable_and_replaced_roots_are_never_indexed() {
    let profile = tempfile::tempdir().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("vault");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("Note.md"), [0xff]).unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), &root).unwrap();
    vault.start_markdown_indexing().await.unwrap();
    until(async || vault.indexing_status().unwrap().error.is_some()).await;
    std::fs::write(root.join("Note.md"), "# Repaired\n").unwrap();
    until(async || {
        vault.indexing_status().unwrap().indexed == 1
            && vault.indexing_status().unwrap().error.is_none()
    })
    .await;
    std::fs::rename(&root, fixture.path().join("original")).unwrap();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("Unadmitted.md"), "# Wrong root\n").unwrap();
    // The retained root watcher reports the move; verify failure rather than
    // granting the replacement path a new capability or scanning it.
    until(async || vault.indexing_status().unwrap().error.is_some()).await;
    close(runtime).await;
    let database = lapis_app_core::database::Database::open(profile.path(), vault.id())
        .await
        .unwrap();
    assert!(
        database
            .get_indexed_file("Unadmitted.md")
            .await
            .unwrap()
            .is_none()
    );
    database.close().await.unwrap();
}

#[tokio::test]
async fn concurrent_start_and_close_drain_without_leaking_a_retired_scheduler() {
    for _ in 0..12 {
        let profile = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("Note.md"), "# Source\n").unwrap();
        let runtime = runtime(&profile);
        let id = Uuid::new_v4();
        let vault = runtime.admit(id, root.path()).unwrap();
        let closing = runtime.clone();
        let (start, close) = tokio::time::timeout(Duration::from_secs(5), async {
            tokio::join!(
                vault.start_markdown_indexing(),
                tokio::task::spawn_blocking(move || closing.close_vault(id))
            )
        })
        .await
        .expect("start/close must not deadlock");
        if let Err(error) = start {
            assert!(matches!(
                error.code(),
                lapis_app_protocol::ErrorCode::Closed | lapis_app_protocol::ErrorCode::Busy
            ));
        }
        close.unwrap().unwrap();
        assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
        assert_eq!(vault.status().unwrap().work, 0);
        assert_eq!(
            vault.start_markdown_indexing().await.unwrap_err().code(),
            lapis_app_protocol::ErrorCode::Closed
        );
        let database = lapis_app_core::database::Database::open(profile.path(), id)
            .await
            .unwrap();
        database.close().await.unwrap();
    }
}

#[tokio::test]
async fn partial_progress_reports_committed_rows_before_a_later_source_error() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a.md"), "# Indexed\n").unwrap();
    std::fs::write(root.path().join("z.md"), [0xff]).unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_markdown_indexing().await.unwrap();
    until(async || vault.indexing_status().unwrap().error.is_some()).await;
    assert_eq!(vault.indexing_status().unwrap().indexed, 1);
    assert!(!row(&vault, "a.md").await.is_null());
    assert!(row(&vault, "z.md").await.is_null());
    std::fs::write(root.path().join("z.md"), "# Repaired\n").unwrap();
    until(async || {
        vault.indexing_status().unwrap().error.is_none()
            && vault.indexing_status().unwrap().indexed == 2
    })
    .await;
    close(runtime).await;
}

#[tokio::test]
async fn property_type_settings_reconcile_without_changing_the_source_note() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("Note.md"),
        "---\nstatus: open\ncheck: false\n---\n# Note\n",
    )
    .unwrap();
    std::fs::create_dir(root.path().join(".lapis")).unwrap();
    let settings = root.path().join(".lapis/types.json");
    std::fs::write(&settings, r#"{"types":{"status":"text"}}"#).unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_markdown_indexing().await.unwrap();
    until(async || vault.indexing_status().unwrap().indexed == 1).await;
    let before = row(&vault, "Note.md").await;
    assert!(
        before["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["name"] == "check" && p["inferredType"] == "checkbox")
    );
    std::fs::write(&settings, r#"{"types":{"status":"multitext"}}"#).unwrap();
    until(async || vault.indexing_status().unwrap().indexed >= 2).await;
    let after = row(&vault, "Note.md").await;
    assert_eq!(before["file"]["hash"], after["file"]["hash"]);
    assert!(
        after["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["name"] == "status" && p["declaredType"] == "multitext")
    );
    close(runtime).await;
}
