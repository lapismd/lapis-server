use super::super::owner::Clock;
use crate::{
    embedding::host::NativeEmbeddingHost,
    services::{DatabaseStorage, Runtime},
};
use lapis_app_protocol::indexing::IndexingCommand;
use std::{
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    task::{Context, Waker},
    time::Duration,
};
use uuid::Uuid;

#[tokio::test]
async fn refresh_returns_its_completed_pass_even_when_the_next_pass_is_active() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let runtime = Arc::new(Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    ));
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let (initial_tx, initial_rx) = mpsc::sync_channel(1);
    let initial_rx = Mutex::new(initial_rx);
    let (next_tx, next_rx) = mpsc::sync_channel(1);
    let next_rx = Mutex::new(next_rx);
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let calls = AtomicUsize::new(0);
    let changed = root.path().join("Next.md");
    vault
        .start_indexing(
            false,
            false,
            Clock {
                today: Arc::new(move || {
                    match calls.fetch_add(1, Ordering::SeqCst) {
                        0 => {
                            initial_rx
                                .lock()
                                .unwrap()
                                .recv_timeout(Duration::from_secs(5))
                                .unwrap();
                        }
                        1 => {
                            std::fs::write(&changed, "# A later watcher pass\n").unwrap();
                        }
                        2 => {
                            entered_tx.send(()).unwrap();
                            next_rx
                                .lock()
                                .unwrap()
                                .recv_timeout(Duration::from_secs(5))
                                .unwrap();
                        }
                        _ => {}
                    }
                    "2026-10-27".into()
                }),
                interval: Duration::from_secs(30),
            },
        )
        .await
        .unwrap();
    // Admit the explicit refresh while the initial pass is held, then deliberately
    // leave its future unpolled until a later watcher pass owns live progress.
    let mut refresh = Box::pin(vault.control_indexing(IndexingCommand::Refresh { rebuild: false }));
    assert!(
        refresh
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    initial_tx.send(()).unwrap();
    tokio::task::spawn_blocking(move || entered_rx.recv_timeout(Duration::from_secs(5)))
        .await
        .unwrap()
        .unwrap();
    let live = vault.indexing_status().unwrap();
    let observed = tokio::time::timeout(Duration::from_secs(5), refresh)
        .await
        .unwrap()
        .unwrap()
        .status
        .unwrap();
    next_tx.send(()).unwrap();
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert!(
        live.active,
        "The deterministic fixture must hold the later pass active"
    );
    assert!(
        !observed.active,
        "Explicit refresh must return its own completed pass, not later live activity"
    );
    assert!(observed.passes < live.passes + 1);
}
