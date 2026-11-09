use super::*;
use crate::{
    config::{Config, VaultConfig},
    http::Authentication,
};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;

fn state() -> (tempfile::TempDir, State) {
    let root = tempfile::tempdir().unwrap();
    let host = Arc::new(
        Host::new(Config {
            vaults: vec![VaultConfig {
                id: Uuid::new_v4(),
                name: "Notes".into(),
                root: root.path().into(),
            }],
        })
        .unwrap(),
    );
    (
        root,
        State {
            host,
            auth: Authentication::token("test-content-owner-0123456789abcdef").unwrap(),
            build_id: "test".into(),
            origin: "https://notes.example".into(),
            downloads: reqwest::Client::new(),
            web: None,
            observations: Arc::new(tokio::sync::Semaphore::new(64)),
        },
    )
}

#[tokio::test]
async fn cancelled_owner_response_keeps_accepted_work_until_shutdown_drains_it() {
    let (_root, state) = state();
    let host = state.host.clone();
    let started = Arc::new(Notify::new());
    let notification = started.clone();
    let ran = Arc::new(AtomicBool::new(false));
    let completed = ran.clone();
    let (release, pending) = tokio::sync::oneshot::channel();
    let caller = tokio::spawn(controlled(state, move |state| async move {
        notification.notify_one();
        pending.await.unwrap();
        blocking(move || state.host.content.grants.set_enabled(true).map_err(native)).await?;
        completed.store(true, Ordering::Release);
        Ok(())
    }));
    started.notified().await;
    caller.abort();
    assert!(caller.await.err().expect("cancelled caller").is_cancelled());
    let closing = host.clone();
    let shutdown = tokio::task::spawn_blocking(move || closing.shutdown());
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !host.content.closed() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!shutdown.is_finished());
    release.send(()).unwrap();
    shutdown.await.unwrap().unwrap();
    assert!(ran.load(Ordering::Acquire));
    assert!(!host.content.grants.is_enabled().unwrap());
    assert!(host.content.grants.set_enabled(true).is_err());
}

#[tokio::test]
async fn owner_control_budget_refuses_excess_work_before_execution() {
    let (_root, state) = state();
    let mut admitted = (0..8)
        .map(|_| state.host.content.accept().unwrap())
        .collect::<Vec<_>>();
    let ran = Arc::new(AtomicBool::new(false));
    let action = ran.clone();
    let failure = controlled(state.clone(), move |_| async move {
        action.store(true, Ordering::Release);
        Ok(())
    })
    .await
    .unwrap_err();
    assert_eq!(failure.1.code, ErrorCode::Busy);
    assert!(!ran.load(Ordering::Acquire));
    admitted.pop();
    assert!(controlled(state, |_| async { Ok(()) }).await.is_ok());
}

#[tokio::test]
async fn refused_native_shutdown_keeps_content_closed_while_owner_can_recover() {
    use crate::host::Principal;
    let (_root, state) = state();
    let host = state.host.clone();
    let vault = host.vaults().unwrap()[0].vault_id;
    let owner = host.attach(vault, Principal::Token).unwrap();
    let client = host.client(owner.client_id, Principal::Token).unwrap();
    let editor = client.open_editor("draft.md").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    host.content.grants.set_enabled(true).unwrap();
    let issued = host
        .issue_content(ContentGrantRequest {
            label: "CLI".into(),
            vault_ids: vec![vault],
            lifetime_seconds: 60,
        })
        .unwrap();
    let closing = host.clone();
    let error = tokio::task::spawn_blocking(move || closing.shutdown())
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Busy);
    assert!(host.ready());
    assert!(host.content.closed());
    assert!(!host.content.grants.is_enabled().unwrap());
    assert!(
        host.content
            .grants
            .check_credential(issued.token.expose_secret())
            .is_err()
    );
    assert!(controlled(state, |_| async { Ok(()) }).await.is_err());
    client.discard_editor(editor).unwrap();
    tokio::task::spawn_blocking(move || host.shutdown())
        .await
        .unwrap()
        .unwrap();
}
