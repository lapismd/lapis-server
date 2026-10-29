use lapis_app_core::services::{BackupStorage, Runtime};
use lapis_app_protocol::repository_restore::*;
use std::{
    fs,
    future::{Future, poll_fn},
    sync::Arc,
    task::Poll,
    time::Duration,
};
use tokio::sync::oneshot;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepted_typed_restore_survives_cancelled_reply_detach_and_vault_drain() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let root = base.join("notes");
    let history = base.join("history");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&history).unwrap();
    fs::write(root.join("Note.md"), "initial saved bytes").unwrap();
    let id = Uuid::new_v4();
    let runtime = Arc::new(Runtime::default().with_backup(BackupStorage::new(&history).unwrap()));
    let vault = runtime.admit(id, &root).unwrap();
    let client = Arc::new(vault.attach().unwrap());
    let status = client.start_backup().await.unwrap();
    let checkpoint = status.borrow().checkpoint.as_ref().unwrap().commit.clone();
    fs::write(root.join("Note.md"), "new saved bytes").unwrap();
    let preview = client
        .control_repository_restore(RepositoryRestoreCommand::Preview {
            checkpoint: checkpoint.clone(),
            path: "Note.md".into(),
        })
        .await
        .unwrap();
    let RepositoryRestoreValue::File(preview) = preview else {
        panic!("expected file preview")
    };
    let request = RepositoryFileRestoreRequest {
        request_id: Uuid::new_v4().to_string(),
        checkpoint,
        path: preview.path,
        expected: preview.expected,
    };
    let (entered, ready) = oneshot::channel();
    let (release, wait) = oneshot::channel();
    let hold_client = client.clone();
    let holding = tokio::spawn(async move {
        hold_client
            .with_backup(move |_| {
                Box::pin(async move {
                    entered.send(()).unwrap();
                    wait.await.unwrap();
                    Ok(())
                })
            })
            .await
    });
    ready.await.unwrap();
    let mut restore = Box::pin(client.control_repository_restore(
        RepositoryRestoreCommand::Apply {
            request: request.clone(),
        },
    ));
    poll_fn(|cx| {
        assert!(restore.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    assert_eq!(vault.status().unwrap().work, 2);
    drop(restore);
    client.detach().unwrap();
    let closing = tokio::task::spawn_blocking(move || runtime.shutdown());
    tokio::time::timeout(Duration::from_secs(5), async {
        while vault.status().unwrap().phase != lapis_app_protocol::VaultPhase::Closing {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!closing.is_finished());
    release.send(()).unwrap();
    holding.await.unwrap().unwrap();
    tokio::time::timeout(Duration::from_secs(10), closing)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.join("Note.md")).unwrap(),
        "initial saved bytes"
    );

    // Explicit retry of the same pending request after a cold reopen returns
    // the retained receipt and cannot overwrite a newer saved file.
    fs::write(root.join("Note.md"), "saved after unknown acknowledgement").unwrap();
    let runtime = Runtime::default().with_backup(BackupStorage::new(&history).unwrap());
    let client = runtime.admit(id, &root).unwrap().attach().unwrap();
    client.start_backup().await.unwrap();
    client
        .control_repository_restore(RepositoryRestoreCommand::Apply { request })
        .await
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.join("Note.md")).unwrap(),
        "saved after unknown acknowledgement"
    );
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
