use super::*;
use crate::config::{Config, VaultConfig};
use lapis_app_protocol::{ErrorCode, repository_restore::RepositoryFileRestoreRequest};
use std::{fs, sync::Arc, time::Duration};
use tokio::sync::oneshot;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retirement_fences_restore_reply_without_canceling_accepted_native_effect() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("notes");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("Note.md"), "initial saved bytes").unwrap();
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
    let client = host.client(descriptor.client_id, Principal::Token).unwrap();
    let status = client.start_backup().await.unwrap();
    let checkpoint = status.borrow().checkpoint.as_ref().unwrap().commit.clone();
    fs::write(root.join("Note.md"), "later saved bytes").unwrap();
    let preview = host
        .repository_restore(
            descriptor.client_id,
            Principal::Token,
            RepositoryRestoreCommand::Preview {
                checkpoint: checkpoint.clone(),
                path: "Note.md".into(),
            },
        )
        .await
        .unwrap();
    let RepositoryRestoreValue::File(preview) = preview else {
        panic!("file preview")
    };
    let request = RepositoryFileRestoreRequest {
        request_id: Uuid::new_v4().to_string(),
        checkpoint,
        path: preview.path,
        expected: preview.expected,
    };
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
    let restoring_host = host.clone();
    let action = RepositoryRestoreCommand::Apply {
        request: request.clone(),
    };
    let restoring = tokio::spawn(async move {
        restoring_host
            .repository_restore(descriptor.client_id, Principal::Token, action)
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
        restoring.await.unwrap().unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        fs::read_to_string(root.join("Note.md")).unwrap(),
        "initial saved bytes"
    );
    fs::write(root.join("Note.md"), "saved after lost acknowledgement").unwrap();
    let next = host.attach(id, Principal::Token).unwrap();
    host.repository_restore(
        next.client_id,
        Principal::Token,
        RepositoryRestoreCommand::Apply { request },
    )
    .await
    .unwrap();
    assert_eq!(
        fs::read_to_string(root.join("Note.md")).unwrap(),
        "saved after lost acknowledgement"
    );
    tokio::task::spawn_blocking(move || host.shutdown())
        .await
        .unwrap()
        .unwrap();
}
