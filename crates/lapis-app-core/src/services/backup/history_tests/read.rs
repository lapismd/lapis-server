use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cold_history_cache_rebuild_with_capture_paused_never_snapshots_disk() {
    let f = fixture(true).await;
    let initial = indexed(&f, "rare initial history").await;
    let checkpoint = initial.hits[0].record.revision.checkpoint.commit.clone();
    let client = f.vault.attach().unwrap();
    client.stop_backup().await.unwrap();
    close(&f).await;
    fs::remove_dir_all(f.binding.state.join("history-search-v2")).unwrap();
    fs::write(f.binding.root.join("Note.md"), "unsnapshotted private edit").unwrap();
    let storage = BackupStorage::new(&f.history)
        .unwrap()
        .with_history_indexing(true);
    let runtime = Arc::new(Runtime::default().with_backup(storage));
    let vault = runtime.admit(f.vault.id(), &f.binding.root).unwrap();
    vault.start_history_indexing().unwrap();
    let first = vault
        .history_indexing
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone();
    vault.start_history_indexing().unwrap();
    assert!(Arc::ptr_eq(
        &first,
        vault.history_indexing.lock().unwrap().as_ref().unwrap()
    ));
    assert!(vault.backup.lock().unwrap().is_none());
    assert_eq!(vault.status().unwrap().clients, 0);
    let rebuilt = indexed(&f, "rare initial history").await;
    assert_eq!(
        rebuilt.hits[0].record.revision.checkpoint.commit,
        checkpoint
    );
    let binding = f.binding.clone();
    let unsnapshotted = tokio::task::spawn_blocking(move || {
        NativeHistory {
            binding,
            exclusions: vec![],
        }
        .indexed_search(&request("unsnapshotted private edit"))
    })
    .await
    .unwrap()
    .unwrap();
    assert!(unsnapshotted.hits.is_empty());
    assert_eq!(
        fs::read_to_string(f.binding.root.join("Note.md")).unwrap(),
        "unsnapshotted private edit"
    );
    let service = vault.clone();
    tokio::task::spawn_blocking(move || service.stop_history_indexing())
        .await
        .unwrap()
        .unwrap();
    assert!(first.is_finished());
    assert_eq!(
        vault.history_indexing_status().unwrap_err().code(),
        lapis_app_protocol::ErrorCode::Unsupported
    );
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_read_admission_never_initializes_a_new_repository() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let root = base.join("notes");
    let history = base.join("history");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&history).unwrap();
    fs::write(root.join("Note.md"), "never captured").unwrap();
    let storage = BackupStorage::new(&history)
        .unwrap()
        .with_history_indexing(true);
    let runtime = Arc::new(Runtime::default().with_backup(storage));
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, &root).unwrap();
    vault.start_history_indexing().unwrap();
    let owner = vault
        .history_indexing
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone();
    wait_progress(&owner, |status| status.error.is_some()).await;
    assert!(vault.backup.lock().unwrap().is_none());
    assert!(!history.join(id.to_string()).exists());
    assert!(!history.join(".layouts").exists());
    assert!(!root.join(".jj").exists());
    assert!(!root.join(".git").exists());
    assert_eq!(
        fs::read_to_string(root.join("Note.md")).unwrap(),
        "never captured"
    );
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert!(owner.is_finished());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_history_rebuilds_after_cold_cache_loss_without_new_checkpoint() {
    let f = fixture(true).await;
    let initial = indexed(&f, "rare initial history").await;
    let checkpoint = initial.hits[0].record.revision.checkpoint.commit.clone();
    let first = owner(&f);
    close(&f).await;
    assert!(first.is_finished());
    fs::remove_dir_all(f.binding.state.join("history-search-v2")).unwrap();
    let storage = BackupStorage::new(&f.history)
        .unwrap()
        .with_history_indexing(true);
    let runtime = Arc::new(Runtime::default().with_backup(storage));
    let vault = runtime.admit(f.vault.id(), &f.binding.root).unwrap();
    let client = vault.attach().unwrap();
    let state = client.start_backup().await.unwrap();
    assert_eq!(
        state.borrow().checkpoint.as_ref().unwrap().commit,
        checkpoint
    );
    drop(client);
    drop(state);
    let rebuilt = indexed(&f, "rare initial history").await;
    assert_eq!(
        rebuilt.hits[0].record.revision.checkpoint.commit,
        checkpoint
    );
    assert_eq!(
        fs::read_to_string(f.binding.root.join("Note.md")).unwrap(),
        "rare initial history text"
    );
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
