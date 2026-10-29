use jj_vault::{Phase, RestoreRequest};
use lapis_app_core::services::{BackupStorage, Client, Runtime};
use std::{
    fs,
    future::{Future, poll_fn},
    sync::{Arc, Mutex},
    task::Poll,
    time::Duration,
};
use tokio::sync::oneshot;
use uuid::Uuid;

struct Fixture {
    _temp: tempfile::TempDir,
    runtime: Arc<Runtime>,
    client: Arc<Client>,
    root: std::path::PathBuf,
}
async fn fixture() -> Fixture {
    fixture_with_files_gate(None).await
}
async fn fixture_with_files_gate(files: Option<Arc<Mutex<()>>>) -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let root = base.join("notes");
    let history = base.join("history");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&history).unwrap();
    fs::write(root.join("Note.md"), "initial").unwrap();
    let storage = BackupStorage::new(&history).unwrap();
    let storage = match files {
        Some(files) => storage.with_files_gate(files),
        None => storage,
    };
    let runtime = Arc::new(Runtime::default().with_backup(storage));
    let client = Arc::new(
        runtime
            .admit(Uuid::new_v4(), &root)
            .unwrap()
            .attach()
            .unwrap(),
    );
    client.start_backup().await.unwrap();
    Fixture {
        _temp: temp,
        runtime,
        client,
        root,
    }
}
async fn close(runtime: Arc<Runtime>) {
    tokio::time::timeout(
        Duration::from_secs(5),
        tokio::task::spawn_blocking(move || runtime.shutdown()),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_disable_and_restart_keep_the_same_retained_history() {
    let f = fixture().await;
    let first = f.client.start_backup().await.unwrap();
    let original = first.borrow().checkpoint.as_ref().unwrap().commit.clone();
    let (a, b) = tokio::join!(f.client.stop_backup(), f.client.stop_backup());
    a.unwrap();
    b.unwrap();
    f.client.stop_backup().await.unwrap();
    assert_eq!(first.borrow().phase, Phase::Closed);
    fs::write(f.root.join("Note.md"), "after explicit restart").unwrap();
    let restarted = f.client.start_backup().await.unwrap();
    assert_ne!(
        restarted.borrow().checkpoint.as_ref().unwrap().commit,
        original
    );
    close(f.runtime).await;
    assert_eq!(restarted.borrow().phase, Phase::Closed);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_disable_retains_the_original_engine_for_repair_and_retry() {
    let f = fixture().await;
    let status = f.client.start_backup().await.unwrap();
    let (entered, ready) = oneshot::channel();
    let (release, gate) = oneshot::channel();
    let client = f.client.clone();
    let holding = tokio::spawn(async move {
        client
            .with_backup(move |_| {
                Box::pin(async move {
                    entered.send(()).unwrap();
                    gate.await.unwrap();
                    Ok(())
                })
            })
            .await
    });
    ready.await.unwrap();
    let client = f.client.clone();
    let stopping = tokio::spawn(async move { client.stop_backup().await });
    let vault = f.runtime.get(f.client.descriptor().vault_id).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while vault.status().unwrap().work != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let away = f.root.with_extension("retained");
    fs::rename(&f.root, &away).unwrap();
    fs::create_dir(&f.root).unwrap();
    fs::write(f.root.join("Note.md"), "replacement root").unwrap();
    release.send(()).unwrap();
    holding.await.unwrap().unwrap();
    let stopped = stopping.await.unwrap();
    let failed = status.borrow().phase.clone();
    fs::remove_dir_all(&f.root).unwrap();
    fs::rename(&away, &f.root).unwrap();
    assert!(stopped.is_err());
    assert_eq!(failed, Phase::Failed);
    let again = f.client.start_backup().await.unwrap();
    f.client.stop_backup().await.unwrap();
    assert_eq!(status.borrow().phase, Phase::Closed);
    assert_eq!(again.borrow().phase, Phase::Closed);
    close(f.runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unpolled_command_reply_cannot_hold_controlled_quit_open() {
    let f = fixture().await;
    let (entered, ready) = oneshot::channel();
    let (release, gate) = oneshot::channel();
    let (completed, done) = oneshot::channel();
    let mut operation = Box::pin(f.client.with_backup(move |engine| {
        Box::pin(async move {
            entered.send(()).unwrap();
            gate.await.unwrap();
            let captured = engine.capture().await;
            completed.send(()).unwrap();
            captured
        })
    }));
    poll_fn(|context| {
        assert!(operation.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
    ready.await.unwrap();
    release.send(()).unwrap();
    done.await.unwrap();
    close(f.runtime.clone()).await;
    drop(operation);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_command_waiter_does_not_cancel_accepted_capture() {
    let f = fixture().await;
    fs::write(f.root.join("Note.md"), "accepted bytes").unwrap();
    let (entered, ready) = oneshot::channel();
    let (release, gate) = oneshot::channel();
    let (completed, done) = oneshot::channel();
    let mut operation = Box::pin(f.client.with_backup(move |engine| {
        Box::pin(async move {
            entered.send(()).unwrap();
            gate.await.unwrap();
            let captured = engine.capture().await;
            completed
                .send(
                    captured
                        .as_ref()
                        .unwrap()
                        .checkpoint
                        .as_ref()
                        .unwrap()
                        .commit
                        .clone(),
                )
                .unwrap();
            captured
        })
    }));
    poll_fn(|context| {
        assert!(operation.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
    ready.await.unwrap();
    drop(operation);
    release.send(()).unwrap();
    let expected = done.await.unwrap();
    let status = f.client.start_backup().await.unwrap();
    assert_eq!(
        status.borrow().checkpoint.as_ref().unwrap().commit,
        expected
    );
    close(f.runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capture_observes_the_host_file_gate_without_holding_the_vault_gate() {
    let files = Arc::new(Mutex::new(()));
    let f = fixture_with_files_gate(Some(files.clone())).await;
    let (entered, ready) = oneshot::channel();
    let client = f.client.clone();
    let (held, acquired) = oneshot::channel();
    let (release, wait) = std::sync::mpsc::channel();
    let host = tokio::task::spawn_blocking(move || {
        let _host_write = files.lock().unwrap();
        held.send(()).unwrap();
        wait.recv().unwrap();
    });
    acquired.await.unwrap();
    let capture = tokio::spawn(async move {
        client
            .with_backup(move |engine| {
                Box::pin(async move {
                    entered.send(()).unwrap();
                    engine.capture().await
                })
            })
            .await
    });
    ready.await.unwrap();
    let blocked = tokio::time::timeout(Duration::from_millis(100), async {
        while !capture.is_finished() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .is_err();
    let client = f.client.clone();
    let read = tokio::time::timeout(
        Duration::from_secs(1),
        tokio::task::spawn_blocking(move || client.read_text("Note.md")),
    )
    .await;
    release.send(()).unwrap();
    host.await.unwrap();
    let captured = tokio::time::timeout(Duration::from_secs(5), capture).await;
    // Release the host gate before any assertion, including failed assertions.
    assert!(blocked, "capture bypassed the host file-publication gate");
    assert_eq!(read.unwrap().unwrap().unwrap().text, "initial");
    captured.unwrap().unwrap().unwrap();
    close(f.runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn existing_restore_runs_through_the_shared_engine_and_protects_dirty_editors() {
    let f = fixture().await;
    let status = f.client.start_backup().await.unwrap();
    let checkpoint = status.borrow().checkpoint.as_ref().unwrap().commit.clone();
    fs::write(f.root.join("Note.md"), "later saved bytes").unwrap();
    let editor = f.client.open_editor("Note.md").unwrap();
    f.client.mark_dirty(editor, 1).unwrap();
    let selected = checkpoint.clone();
    let blocked = f
        .client
        .with_backup(move |engine| {
            Box::pin(async move {
                let preview = engine
                    .preview_restore(selected.clone(), "Note.md".into())
                    .await?;
                engine
                    .restore(RestoreRequest {
                        request_id: Uuid::new_v4().to_string(),
                        checkpoint: selected,
                        path: preview.path,
                        expected: preview.expected,
                    })
                    .await
            })
        })
        .await;
    assert!(blocked.is_err());
    assert_eq!(
        fs::read_to_string(f.root.join("Note.md")).unwrap(),
        "later saved bytes"
    );
    f.client.discard_editor(editor).unwrap();
    f.client
        .with_backup(move |engine| {
            Box::pin(async move {
                let preview = engine
                    .preview_restore(checkpoint.clone(), "Note.md".into())
                    .await?;
                engine
                    .restore(RestoreRequest {
                        request_id: Uuid::new_v4().to_string(),
                        checkpoint,
                        path: preview.path,
                        expected: preview.expected,
                    })
                    .await
            })
        })
        .await
        .unwrap();
    assert_eq!(
        fs::read_to_string(f.root.join("Note.md")).unwrap(),
        "initial"
    );
    close(f.runtime).await;
}
