use lapis_app_core::{
    database::Database,
    embedding::host::NativeEmbeddingHost,
    services::{DatabaseStorage, Runtime},
};
use lapis_app_protocol::{
    ErrorCode,
    database::call::{DatabaseCall, DatabaseMethod, DatabaseValue},
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use uuid::Uuid;

fn set(index: usize) -> DatabaseCall {
    DatabaseCall {
        method: DatabaseMethod::SetMeta,
        args: vec![
            DatabaseValue::Value {
                value: json!("key"),
            },
            DatabaseValue::Value {
                value: json!(index),
            },
        ],
    }
}
fn selected(profile: &tempfile::TempDir) -> Arc<Runtime> {
    Arc::new(Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    ))
}

#[tokio::test]
async fn passive_subscriptions_share_commits_and_never_own_vault_close() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let runtime = selected(&profile);
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, root.path()).unwrap();
    let first = vault.attach().unwrap();
    let second = vault.attach().unwrap();
    let mut a = first.subscribe_to_database_changes().await.unwrap();
    let mut b = second.subscribe_to_database_changes().await.unwrap();
    assert_eq!(vault.status().unwrap().work, 0);
    first.call_database(set(1)).await.unwrap();
    let change = a.recv().await.unwrap();
    assert_eq!(change.revision, 1);
    assert!(change.paths.is_empty());
    assert_eq!(
        serde_json::to_value(&change.domains).unwrap(),
        json!(["meta"])
    );
    assert_eq!(b.recv().await.unwrap().revision, change.revision);
    drop(first);
    drop(second);
    assert_eq!(vault.status().unwrap().clients, 0);
    let native = vault.begin_work().unwrap();
    native.call_database(set(2)).await.unwrap();
    drop(native);
    assert_eq!(a.recv().await.unwrap().revision, 2);
    assert_eq!(b.recv().await.unwrap().revision, 2);
    tokio::time::timeout(
        Duration::from_secs(5),
        tokio::task::spawn_blocking(move || runtime.close_vault(id)),
    )
    .await
    .expect("passive receivers cannot keep vault close open")
    .unwrap()
    .unwrap();
    assert_eq!(a.recv().await.unwrap_err(), RecvError::Closed);
    assert_eq!(b.try_recv().unwrap_err(), TryRecvError::Closed);
    let reopened = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(reopened.get_meta("key").await.unwrap(), Some(json!(2)));
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn a_slow_client_observes_bounded_lag_without_blocking_writes_or_quit() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let runtime = selected(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    let mut changes = client.subscribe_to_database_changes().await.unwrap();
    for index in 0..130 {
        client.call_database(set(index)).await.unwrap();
    }
    assert_eq!(changes.try_recv().unwrap_err(), TryRecvError::Lagged(2));
    assert_eq!(changes.recv().await.unwrap().revision, 3);
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
    while changes.recv().await.is_ok() {}
    assert_eq!(changes.try_recv().unwrap_err(), TryRecvError::Closed);
    assert_eq!(
        client
            .subscribe_to_database_changes()
            .await
            .err()
            .unwrap()
            .code(),
        ErrorCode::Closed
    );
}

#[tokio::test]
async fn subscriptions_retain_native_selection_and_physical_root_checks() {
    let root = tempfile::tempdir().unwrap();
    let file_only = Runtime::default();
    let vault = file_only.admit(Uuid::new_v4(), root.path()).unwrap();
    assert_eq!(
        vault
            .attach()
            .unwrap()
            .subscribe_to_database_changes()
            .await
            .err()
            .unwrap()
            .code(),
        ErrorCode::Unsupported
    );
    file_only.shutdown().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let original = root.path().join("vault");
    std::fs::create_dir(&original).unwrap();
    let runtime = selected(&profile);
    let vault = runtime.admit(Uuid::new_v4(), &original).unwrap();
    let client = vault.attach().unwrap();
    std::fs::rename(&original, root.path().join("moved")).unwrap();
    std::fs::create_dir(&original).unwrap();
    assert!(client.subscribe_to_database_changes().await.is_err());
    assert_eq!(std::fs::read_dir(profile.path()).unwrap().count(), 0);
    runtime.shutdown().unwrap();
}
