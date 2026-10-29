use crate::services::{BackupStorage, Runtime};
use std::{
    fs,
    sync::{Arc, mpsc},
    time::Duration,
};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_initialization_keeps_accepted_work_owned_until_close_drains_it() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let root = base.join("notes");
    let history = base.join("history");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&history).unwrap();
    fs::write(root.join("Note.md"), "accepted saved bytes").unwrap();
    let runtime = Arc::new(Runtime::default().with_backup(BackupStorage::new(&history).unwrap()));
    let vault = runtime.admit(Uuid::new_v4(), &root).unwrap();
    let client = Arc::new(vault.attach().unwrap());
    let (entered, ready) = tokio::sync::oneshot::channel();
    let (release, gate) = mpsc::channel();
    let held_vault = vault.clone();
    let holding = tokio::task::spawn_blocking(move || {
        let _initialization = held_vault.backup.lock().unwrap();
        entered.send(()).unwrap();
        gate.recv().unwrap();
    });
    ready.await.unwrap();
    let task_client = client.clone();
    let start = tokio::spawn(async move { task_client.start_backup().await });
    tokio::time::timeout(Duration::from_secs(5), async {
        while vault.status().unwrap().work == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    start.abort();
    assert!(start.await.unwrap_err().is_cancelled());
    assert_eq!(vault.status().unwrap().work, 1);
    let close = tokio::task::spawn_blocking(move || runtime.shutdown());
    tokio::time::timeout(Duration::from_secs(5), async {
        while vault.status().unwrap().phase != lapis_app_protocol::VaultPhase::Closing {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!close.is_finished());
    release.send(()).unwrap();
    holding.await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), close)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        vault.status().unwrap().phase,
        lapis_app_protocol::VaultPhase::Closed
    );
}
