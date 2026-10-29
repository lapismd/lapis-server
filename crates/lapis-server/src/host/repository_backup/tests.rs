use super::*;
use crate::config::{Config, VaultConfig};
use lapis_app_protocol::ErrorCode;
use std::{fs, sync::Arc, time::Duration};
use tokio::sync::oneshot;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retirement_fences_selection_acknowledgement_but_keeps_the_saved_disable() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("notes");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("Note.md"), "retained bytes").unwrap();
    let id = Uuid::new_v4();
    let profile = tempfile::tempdir_in(temp.path()).unwrap();
    let host = Arc::new(
        Host::with_profile(
            Config {
                vaults: vec![VaultConfig {
                    id,
                    name: "Notes".into(),
                    root: root.clone(),
                }],
            },
            profile.path(),
        )
        .unwrap(),
    );
    let descriptor = host.attach(id, Principal::Token).unwrap();
    host.prepare_client(descriptor.client_id, Principal::Token)
        .await
        .unwrap();
    let before = host
        .repository_backup(
            descriptor.client_id,
            Principal::Token,
            RepositoryBackupCommand::Status {},
        )
        .await
        .unwrap();
    let client = host.client(descriptor.client_id, Principal::Token).unwrap();
    let (entered, ready) = oneshot::channel();
    let (release, wait) = oneshot::channel();
    let holding = tokio::spawn(async move {
        client
            .with_backup(move |_| {
                Box::pin(async move {
                    entered.send(()).unwrap();
                    wait.await.unwrap();
                    Ok(())
                })
            })
            .await
            .unwrap();
    });
    ready.await.unwrap();
    let selecting_host = host.clone();
    let selecting = tokio::spawn(async move {
        selecting_host
            .repository_backup(
                descriptor.client_id,
                Principal::Token,
                RepositoryBackupCommand::SetEnabled {
                    enabled: false,
                    expected: before.configuration,
                },
            )
            .await
    });
    let vault = host.runtime.get(id).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while vault.status().unwrap().work != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    host.detach(descriptor.client_id, Principal::Token).unwrap();
    release.send(()).unwrap();
    holding.await.unwrap();
    assert_eq!(
        selecting.await.unwrap().unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    let next = host.attach(id, Principal::Token).unwrap();
    host.prepare_client(next.client_id, Principal::Token)
        .await
        .unwrap();
    let saved = host
        .repository_backup(
            next.client_id,
            Principal::Token,
            RepositoryBackupCommand::Status {},
        )
        .await
        .unwrap();
    assert!(!saved.enabled);
    assert!(matches!(
        saved.status.phase,
        lapis_app_protocol::repository_backup::RepositoryBackupPhase::Closed
    ));
    assert_eq!(
        fs::read_to_string(root.join("Note.md")).unwrap(),
        "retained bytes"
    );
    tokio::task::spawn_blocking(move || host.shutdown())
        .await
        .unwrap()
        .unwrap();
}
