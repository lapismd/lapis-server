mod concurrency;
mod controls;
mod read;

use super::*;
use crate::services::{Runtime, VaultService};
use jj_vault::{
    Binding,
    search::{
        NativeHistory,
        indexed::{Expression, Request},
    },
};
use std::{fs, path::PathBuf, time::Duration};
use uuid::Uuid;

struct Fixture {
    _temp: tempfile::TempDir,
    runtime: Arc<Runtime>,
    vault: Arc<VaultService>,
    binding: Binding,
    history: PathBuf,
}
async fn fixture(selected: bool) -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let root = base.join("notes");
    let history = base.join("history");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&history).unwrap();
    fs::write(root.join("Note.md"), "rare initial history text").unwrap();
    let id = Uuid::new_v4();
    let storage = BackupStorage::new(&history).unwrap();
    let storage = if selected {
        storage.with_history_indexing(true)
    } else {
        storage
    };
    let runtime = Arc::new(Runtime::default().with_backup(storage));
    let vault = runtime.admit(id, &root).unwrap();
    let client = vault.attach().unwrap();
    client.start_backup().await.unwrap();
    drop(client);
    Fixture {
        _temp: temp,
        runtime,
        vault,
        binding: Binding {
            vault_id: id.to_string(),
            root,
            state: history.join(id.to_string()),
        },
        history,
    }
}
fn request(value: &str) -> Request {
    Request {
        expression: Expression::Term {
            field: Some("content".into()),
            value: value.into(),
        },
        case_sensitive: false,
        cursor: None,
        limit: 100,
    }
}
async fn indexed(f: &Fixture, value: &str) -> jj_vault::search::indexed::Page {
    let binding = f.binding.clone();
    let value = value.to_owned();
    tokio::time::timeout(Duration::from_secs(12), async move {
        loop {
            // Query must never create the cache or perform indexing on behalf
            // of the missing owner. Only read after background publication.
            if binding.state.join("history-search-v2").is_dir() {
                let source = binding.clone();
                let query = request(&value);
                let result = tokio::task::spawn_blocking(move || {
                    NativeHistory {
                        binding: source,
                        exclusions: vec![],
                    }
                    .indexed_search(&query)
                })
                .await
                .unwrap();
                match result {
                    Ok(page) if page.status.complete && !page.hits.is_empty() => return page,
                    Ok(_) => {}
                    Err(error) if error == "History index is busy" => {}
                    Err(error) => panic!("immutable History query failed: {error}"),
                }
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    })
    .await
    .expect("selected service must index without a client step request")
}
async fn close(f: &Fixture) {
    let runtime = f.runtime.clone();
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_history_index_publishes_without_a_client_or_renderer_step() {
    let f = fixture(true).await;
    assert_eq!(f.vault.status().unwrap().clients, 0);
    let page = indexed(&f, "rare initial history").await;
    assert!(
        page.hits
            .iter()
            .any(|hit| hit.record.path.as_deref() == Some("Note.md"))
    );
    assert!(f.vault.history_indexing_status().unwrap().passes > 0);
    close(&f).await;
    assert_eq!(
        f.vault.history_indexing_status().unwrap_err().code(),
        lapis_app_protocol::ErrorCode::Closed
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unselected_backup_keeps_history_indexing_off() {
    let f = fixture(false).await;
    assert_eq!(
        f.vault.history_indexing_status().unwrap_err().code(),
        lapis_app_protocol::ErrorCode::Unsupported
    );
    close(&f).await;
    assert!(!f.binding.state.join("history-search-v2").exists());
}

fn owner(f: &Fixture) -> Arc<history::Owner> {
    f.vault
        .history_indexing
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone()
}
async fn wait_progress(
    owner: &history::Owner,
    ready: impl Fn(&history::HistoryIndexStatus) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            if ready(&owner.status.lock().unwrap()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_history_follows_external_capture_with_no_client() {
    let f = fixture(true).await;
    let initial = indexed(&f, "rare initial history").await;
    let previous = initial.hits[0].record.revision.checkpoint.commit.clone();
    fs::write(
        f.binding.root.join("Note.md"),
        "external captured history phrase",
    )
    .unwrap();
    let changed = indexed(&f, "external captured history").await;
    assert_ne!(changed.hits[0].record.revision.checkpoint.commit, previous);
    assert_eq!(f.vault.status().unwrap().clients, 0);
    assert_eq!(
        fs::read_to_string(f.binding.root.join("Note.md")).unwrap(),
        "external captured history phrase"
    );
    close(&f).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_history_pause_retains_owner_and_restart_updates_checkpoint() {
    let f = fixture(true).await;
    let initial = indexed(&f, "rare initial history").await;
    let previous = initial.hits[0].record.revision.checkpoint.commit.clone();
    let first = owner(&f);
    let client = f.vault.attach().unwrap();
    client.stop_backup().await.unwrap();
    assert!(!first.is_finished());
    assert!(f.vault.backup.lock().unwrap().is_none());
    let retained = indexed(&f, "rare initial history").await;
    assert_eq!(retained.hits[0].record.revision.checkpoint.commit, previous);
    fs::write(
        f.binding.root.join("Note.md"),
        "saved while capture was disabled",
    )
    .unwrap();
    client.start_backup().await.unwrap();
    let second = owner(&f);
    assert!(Arc::ptr_eq(&first, &second));
    assert!(!second.is_finished());
    drop(client);
    let resumed = indexed(&f, "saved while capture").await;
    assert_ne!(resumed.hits[0].record.revision.checkpoint.commit, previous);
    close(&f).await;
    assert!(second.is_finished());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_history_rejects_replaced_roots_and_retains_owner_for_close_retry() {
    let f = fixture(true).await;
    indexed(&f, "rare initial history").await;
    let first = owner(&f);
    let client = f.vault.attach().unwrap();
    let retained = f.binding.root.with_extension("retained");
    fs::rename(&f.binding.root, &retained).unwrap();
    fs::create_dir(&f.binding.root).unwrap();
    fs::write(f.binding.root.join("Note.md"), "unadmitted replacement").unwrap();
    let runtime = f.runtime.clone();
    let id = f.vault.id();
    assert!(
        tokio::task::spawn_blocking(move || runtime.close_vault(id))
            .await
            .unwrap()
            .is_err()
    );
    wait_progress(&first, |status| status.error.is_some()).await;
    assert!(Arc::ptr_eq(&first, &owner(&f)));
    assert!(!first.is_finished());
    assert_eq!(
        fs::read_to_string(f.binding.root.join("Note.md")).unwrap(),
        "unadmitted replacement"
    );
    fs::remove_dir_all(&f.binding.root).unwrap();
    fs::rename(&retained, &f.binding.root).unwrap();
    client
        .with_backup(|engine| Box::pin(async move { engine.capture().await }))
        .await
        .unwrap();
    wait_progress(&first, |status| status.error.is_none()).await;
    drop(client);
    close(&f).await;
    assert!(first.is_finished());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_history_two_vaults_keep_independent_owners_and_close() {
    let a = fixture(true).await;
    let b = fixture(true).await;
    indexed(&a, "rare initial history").await;
    indexed(&b, "rare initial history").await;
    let first = owner(&a);
    let second = owner(&b);
    assert!(!Arc::ptr_eq(&first, &second));
    close(&a).await;
    assert!(first.is_finished());
    fs::write(
        b.binding.root.join("Note.md"),
        "second independent vault edit",
    )
    .unwrap();
    indexed(&b, "second independent vault").await;
    assert!(!second.is_finished());
    close(&b).await;
    assert!(second.is_finished());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn paused_capture_retains_the_native_history_index_owner() {
    let f = fixture(true).await;
    indexed(&f, "rare initial history").await;
    let client = f.vault.attach().unwrap();
    client.stop_backup().await.unwrap();
    assert!(f.vault.backup.lock().unwrap().is_none());
    let status = f.vault.history_indexing_status();
    close(&f).await;
    assert!(
        status.is_ok(),
        "paused capture must retain History: {status:?}"
    );
}
