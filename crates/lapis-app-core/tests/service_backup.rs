use jj_vault::{Binding, Phase, Repository, RestoreRequest};
use lapis_app_core::services::{BackupStorage, Runtime};
use std::{fs, sync::Arc, time::Duration};
use uuid::Uuid;

struct Fixture {
    _temp: tempfile::TempDir,
    runtime: Arc<Runtime>,
    id: Uuid,
    binding: Binding,
}
fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let root = base.join("notes");
    let history = base.join("history");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&history).unwrap();
    fs::write(root.join("Note.md"), "initial note").unwrap();
    let id = Uuid::new_v4();
    let runtime = Arc::new(Runtime::default().with_backup(BackupStorage::new(&history).unwrap()));
    runtime.admit(id, &root).unwrap();
    Fixture {
        _temp: temp,
        runtime,
        id,
        binding: Binding {
            vault_id: id.to_string(),
            root,
            state: history.join(id.to_string()),
        },
    }
}
async fn saved_checkpoint_after(binding: &Binding, previous: &str) -> String {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let reader = jj_store_jj::vault_workspace::HistoryReader::open(binding.clone())
                .await
                .unwrap();
            let page = reader.history_revision(None, 0).await.unwrap();
            if let Some(revision) = page.revision
                && revision.checkpoint.commit != previous
            {
                return revision.checkpoint.commit;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap()
}
fn restore_and_assert(binding: Binding, checkpoint: String, expected: &str) {
    let mut repository = jj_vault::NativeRepository {
        binding: binding.clone(),
        exclusions: vec![],
    };
    fs::write(binding.root.join("Note.md"), "later disposable edit").unwrap();
    let preview = repository.preview_restore(&checkpoint, "Note.md").unwrap();
    repository
        .restore(&RestoreRequest {
            request_id: Uuid::new_v4().to_string(),
            checkpoint,
            path: "Note.md".into(),
            expected: preview.expected,
        })
        .unwrap();
    assert_eq!(
        fs::read_to_string(binding.root.join("Note.md")).unwrap(),
        expected
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_watch_captures_after_client_detach_and_without_a_live_observer() {
    let f = fixture();
    let client = f.runtime.get(f.id).unwrap().attach().unwrap();
    let state = client.start_backup().await.unwrap();
    let first = state.borrow().checkpoint.as_ref().unwrap().commit.clone();
    drop(state);
    drop(client);
    fs::write(f.binding.root.join("Note.md"), "saved without a client").unwrap();
    let captured = saved_checkpoint_after(&f.binding, &first).await;
    let later = f.runtime.get(f.id).unwrap().attach().unwrap();
    let state = later.start_backup().await.unwrap();
    assert_eq!(state.borrow().checkpoint.as_ref().unwrap().commit, captured);
    drop(later);
    let runtime = f.runtime.clone();
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
    tokio::task::spawn_blocking(move || {
        restore_and_assert(f.binding, captured, "saved without a client")
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_final_capture_keeps_the_original_owner_for_repaired_close() {
    let f = fixture();
    let client = f.runtime.get(f.id).unwrap().attach().unwrap();
    let state = client.start_backup().await.unwrap();
    let away = f.binding.root.with_extension("retained");
    fs::rename(&f.binding.root, &away).unwrap();
    fs::create_dir(&f.binding.root).unwrap();
    fs::write(f.binding.root.join("Note.md"), "replacement root").unwrap();
    let runtime = f.runtime.clone();
    let id = f.id;
    assert!(
        tokio::task::spawn_blocking(move || runtime.close_vault(id))
            .await
            .unwrap()
            .is_err()
    );
    assert_eq!(state.borrow().phase, Phase::Failed);
    fs::remove_dir_all(&f.binding.root).unwrap();
    fs::rename(&away, &f.binding.root).unwrap();
    fs::write(f.binding.root.join("Note.md"), "repaired saved bytes").unwrap();
    let again = client.start_backup().await.unwrap();
    let runtime = f.runtime.clone();
    tokio::task::spawn_blocking(move || runtime.close_vault(id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.borrow().phase, Phase::Closed);
    assert_eq!(again.borrow().phase, Phase::Closed);
    let checkpoint = state.borrow().checkpoint.as_ref().unwrap().commit.clone();
    tokio::task::spawn_blocking(move || {
        restore_and_assert(f.binding, checkpoint, "repaired saved bytes")
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn simultaneous_clients_share_one_coordinator_and_passive_receivers_do_not_hold_close() {
    let f = fixture();
    let a = f.runtime.get(f.id).unwrap().attach().unwrap();
    let b = f.runtime.get(f.id).unwrap().attach().unwrap();
    let (a_state, b_state) = tokio::join!(a.start_backup(), b.start_backup());
    let a_state = a_state.unwrap();
    let b_state = b_state.unwrap();
    assert_eq!(
        a_state.borrow().checkpoint.as_ref().unwrap().commit,
        b_state.borrow().checkpoint.as_ref().unwrap().commit
    );
    let runtime = f.runtime.clone();
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(a_state.borrow().phase, Phase::Closed);
    assert_eq!(b_state.borrow().phase, Phase::Closed);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn controlled_quit_captures_the_last_saved_bytes_and_releases_native_history() {
    let f = fixture();
    let client = f.runtime.get(f.id).unwrap().attach().unwrap();
    let state = client.start_backup().await.unwrap();
    fs::write(f.binding.root.join("Note.md"), "final saved bytes").unwrap();
    drop(client);
    let runtime = f.runtime.clone();
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
    let final_status = state.borrow().clone();
    assert_eq!(final_status.phase, Phase::Closed);
    let checkpoint = final_status.checkpoint.unwrap().commit;
    tokio::task::spawn_blocking(move || {
        restore_and_assert(f.binding, checkpoint, "final saved bytes")
    })
    .await
    .unwrap();
}

#[test]
fn external_history_storage_cannot_be_admitted_inside_the_vault() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let history = base.join("history");
    fs::create_dir(&history).unwrap();
    let runtime = Runtime::default().with_backup(BackupStorage::new(&history).unwrap());
    assert_eq!(
        runtime.admit(Uuid::new_v4(), &base).err().unwrap().code(),
        lapis_app_protocol::ErrorCode::PermissionDenied
    );
    assert!(fs::read_dir(&history).unwrap().next().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cached_backup_admission_rejects_a_replaced_history_directory() {
    let f = fixture();
    let client = f.runtime.get(f.id).unwrap().attach().unwrap();
    let state = client.start_backup().await.unwrap();
    let history = f.binding.state.parent().unwrap();
    let retained = history.with_extension("retained");
    fs::rename(history, &retained).unwrap();
    fs::create_dir(history).unwrap();
    let result = client.start_backup().await;
    assert!(fs::read_dir(history).unwrap().next().is_none());
    fs::remove_dir(history).unwrap();
    fs::rename(&retained, history).unwrap();
    let runtime = f.runtime.clone();
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.borrow().phase, Phase::Closed);
    assert!(
        result.is_err(),
        "cached backup views must revalidate native history storage"
    );
}
