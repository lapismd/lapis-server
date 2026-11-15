use lapis_app_core::services::{
    BackupStorage, RemoteCredentialAction, RemoteCredentialFuture, RemoteCredentials,
    RemoteStorage, Runtime,
};
use lapis_app_protocol::{ErrorCode, repository_remotes::*};
use std::{
    fs,
    os::unix::fs::PermissionsExt as _,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use uuid::Uuid;

struct Custody(Mutex<RemoteSettings>, AtomicBool);
struct Paused {
    custody: Arc<Custody>,
    entered: std::sync::mpsc::Sender<()>,
    resume: tokio::sync::Notify,
}
impl RemoteCredentials for Paused {
    fn inventory(&self) -> lapis_app_core::services::Result<RemoteSettings> {
        self.custody.inventory()
    }
    fn control(&self, action: RemoteCredentialAction) -> RemoteCredentialFuture<'_> {
        Box::pin(async move {
            self.entered.send(()).unwrap();
            self.resume.notified().await;
            self.custody.control(action).await
        })
    }
}
impl RemoteCredentials for Custody {
    fn inventory(&self) -> lapis_app_core::services::Result<RemoteSettings> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn control(&self, action: RemoteCredentialAction) -> RemoteCredentialFuture<'_> {
        Box::pin(async move {
            match action {
                RemoteCredentialAction::Save(input) => {
                    let mut settings = self.0.lock().unwrap();
                    if let Some(id) = input.replace {
                        settings
                            .credentials
                            .iter_mut()
                            .find(|c| c.id == id)
                            .unwrap()
                            .label = input.label;
                    } else {
                        settings.credentials.push(RemoteCredentialSummary {
                            id: Uuid::new_v4().to_string(),
                            label: input.label,
                            host: input.host,
                            username: input.username,
                            method: RemoteCredentialMethod::Token,
                            available: true,
                        });
                    }
                }
                RemoteCredentialAction::Forget(id) => {
                    self.0.lock().unwrap().credentials.retain(|c| c.id != id);
                }
                _ => unreachable!(),
            }
            if self.1.swap(false, Ordering::SeqCst) {
                return Err(std::io::Error::other("Fixture loses the custody response").into());
            }
            Ok(())
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pending_backup_keeps_its_credential_across_vaults_until_local_stop() {
    use jj_store_jj::vault_workspace::HistoryReader;
    use jj_vault::{Binding, RemoteTransfer, Storage};
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    for name in ["private", "first", "second", "history"] {
        fs::create_dir(base.join(name)).unwrap();
        fs::set_permissions(base.join(name), fs::Permissions::from_mode(0o700)).unwrap();
    }
    let token_id = Uuid::new_v4().to_string();
    let custody = Arc::new(Custody(
        Mutex::new(RemoteSettings {
            host: RemoteHost::Server,
            storage: RemoteStorageState::Available,
            ssh_available: false,
            device_sign_in_available: false,
            authorization: None,
            targets: vec![],
            automatic_sync: false,
            credentials: vec![RemoteCredentialSummary {
                id: token_id.clone(),
                label: "Retained token".into(),
                host: "example.invalid".into(),
                username: "fixture".into(),
                method: RemoteCredentialMethod::Token,
                available: true,
            }],
        }),
        AtomicBool::new(false),
    ));
    let runtime = Arc::new(
        Runtime::default()
            .with_backup(BackupStorage::new(&base.join("history")).unwrap())
            .with_remotes(RemoteStorage::new(&base.join("private"), custody.clone()).unwrap()),
    );
    let id = Uuid::new_v4();
    let root = base.join("first");
    fs::write(root.join("Note.md"), "saved locally\n").unwrap();
    let first = runtime.admit(id, &root).unwrap();
    let second = runtime.admit(Uuid::new_v4(), &base.join("second")).unwrap();
    let client = first.attach().unwrap();
    let other = second.attach().unwrap();
    let status = client.start_backup().await.unwrap();
    let connection = RemoteConnection {
        name: "backup".into(),
        url: "https://example.invalid/notes.git".into(),
        branch: "backup".into(),
        bookmark: "lapis-backup".into(),
        authentication: RemoteAuthentication::Token,
        credential: token_id.clone(),
    };
    let selected = client
        .control_repository_remotes(RepositoryRemotesCommand::SaveConnection {
            expected_revision: 0,
            target: RemoteRepositoryTarget::Backup,
            connection: connection.clone(),
        })
        .await
        .unwrap();
    let binding = Storage::new(
        Binding {
            vault_id: id.to_string(),
            root: root.clone(),
            state: base.join("history").join(id.to_string()),
        },
        base.join("history/.layouts").join(id.to_string()),
    )
    .binding()
    .unwrap();
    let remote = base.join("fixture.git");
    assert!(
        std::process::Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(&remote)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .unwrap()
            .success()
    );
    let transfer = RemoteTransfer::connect_local(binding.clone(), &remote, "backup")
        .await
        .unwrap();
    let reader = HistoryReader::open(binding).await.unwrap();
    let request_id = Uuid::new_v4().to_string();
    let native = reader
        .prepare_push(&transfer, &request_id, None)
        .await
        .unwrap();
    let counts = reader.push_divergence(&native).await.unwrap();
    transfer.reserve(&native).unwrap();
    // A native prepared receipt plus an application hold. The deliberately old
    // source witness must not let credential mutation discard its protection.
    let journal = base
        .join("private")
        .join(id.to_string())
        .join("backup-review.json");
    fs::write(
        &journal,
        serde_json::to_vec(&serde_json::json!({
            "version":1, "source":"0".repeat(64), "connection":connection,
            "observation":null, "review":{"native":native,"counts":counts,"started":true},
        }))
        .unwrap(),
    )
    .unwrap();
    let stale = other.repository_remote_settings().await.unwrap();
    assert_eq!(
        other
            .control_repository_remotes(RepositoryRemotesCommand::ForgetCredential {
                expected_revision: stale.revision,
                id: token_id.clone(),
            })
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Busy
    );
    assert_eq!(custody.0.lock().unwrap().credentials.len(), 1);
    let stopped = client
        .control_repository_remotes(RepositoryRemotesCommand::StopPush {
            expected_revision: selected.revision,
            target: RemoteRepositoryTarget::Backup,
            request_id: request_id.clone(),
        })
        .await
        .unwrap();
    assert!(stopped.settings.targets[0].publication.is_none());
    let forgotten = other
        .control_repository_remotes(RepositoryRemotesCommand::ForgetCredential {
            expected_revision: stale.revision,
            id: token_id,
        })
        .await
        .unwrap();
    assert!(forgotten.settings.credentials.is_empty());
    let current = client.repository_remote_settings().await.unwrap();
    assert_eq!(
        client
            .control_repository_remotes(RepositoryRemotesCommand::Push {
                expected_revision: current.revision,
                target: RemoteRepositoryTarget::Backup,
                review_id: request_id,
            })
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Conflict
    );
    assert_eq!(
        fs::read_to_string(root.join("Note.md")).unwrap(),
        "saved locally\n"
    );
    assert!(!root.join(".jj").exists());
    drop(status);
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn credential_mutation_fences_every_vault_without_persisting_secret_inputs() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    for name in ["private", "first", "second"] {
        fs::create_dir(base.join(name)).unwrap();
        fs::set_permissions(base.join(name), fs::Permissions::from_mode(0o700)).unwrap();
    }
    let custody = Arc::new(Custody(
        Mutex::new(RemoteSettings {
            host: RemoteHost::Server,
            storage: RemoteStorageState::Available,
            ssh_available: false,
            device_sign_in_available: false,
            authorization: None,
            credentials: vec![],
            targets: vec![],
            automatic_sync: false,
        }),
        AtomicBool::new(false),
    ));
    let runtime = Arc::new(
        Runtime::default()
            .with_remotes(RemoteStorage::new(&base.join("private"), custody.clone()).unwrap()),
    );
    let first = runtime.admit(Uuid::new_v4(), &base.join("first")).unwrap();
    let second = runtime.admit(Uuid::new_v4(), &base.join("second")).unwrap();
    let client = first.attach().unwrap();
    let other = second.attach().unwrap();
    let initial = client.repository_remote_settings().await.unwrap();
    let stale = other.repository_remote_settings().await.unwrap();
    let saved = client
        .control_repository_remotes(RepositoryRemotesCommand::SaveCredential {
            expected_revision: initial.revision,
            input: RemoteCredentialInput {
                label: "My repository".into(),
                host: "example.invalid".into(),
                username: "fixture".into(),
                token: "private-test-token".into(),
                provider: RemoteCredentialProvider::Https,
                replace: None,
            },
        })
        .await
        .unwrap();
    assert!(saved.revision > initial.revision);
    let id = saved.settings.credentials[0].id.clone();
    assert!(
        !serde_json::to_string(&saved)
            .unwrap()
            .contains("private-test-token")
    );
    assert_eq!(
        other
            .control_repository_remotes(RepositoryRemotesCommand::ForgetCredential {
                expected_revision: stale.revision,
                id: id.clone(),
            })
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Conflict
    );
    assert_eq!(custody.0.lock().unwrap().credentials.len(), 1);
    let refreshed = other.repository_remote_settings().await.unwrap();
    assert!(refreshed.revision > stale.revision);
    // Scope changes refuse before custody and before reserving a new epoch.
    let error = other
        .control_repository_remotes(RepositoryRemotesCommand::SaveCredential {
            expected_revision: refreshed.revision,
            input: RemoteCredentialInput {
                label: "Different host".into(),
                host: "other.invalid".into(),
                username: "fixture".into(),
                token: "replacement-test-token".into(),
                provider: RemoteCredentialProvider::Https,
                replace: Some(id.clone()),
            },
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidRequest);
    assert_eq!(
        other.repository_remote_settings().await.unwrap().revision,
        refreshed.revision
    );
    custody.1.store(true, Ordering::SeqCst);
    let error = other
        .control_repository_remotes(RepositoryRemotesCommand::SaveCredential {
            expected_revision: refreshed.revision,
            input: RemoteCredentialInput {
                label: "Replacement".into(),
                host: "example.invalid".into(),
                username: "fixture".into(),
                token: "replacement-test-token".into(),
                provider: RemoteCredentialProvider::Https,
                replace: Some(id.clone()),
            },
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::Io);
    assert_eq!(
        other
            .control_repository_remotes(RepositoryRemotesCommand::ForgetCredential {
                expected_revision: refreshed.revision,
                id: id.clone(),
            })
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Conflict
    );
    let refreshed = other.repository_remote_settings().await.unwrap();
    assert_eq!(refreshed.settings.credentials[0].id, id);
    let forgotten = other
        .control_repository_remotes(RepositoryRemotesCommand::ForgetCredential {
            expected_revision: refreshed.revision,
            id,
        })
        .await
        .unwrap();
    assert!(forgotten.settings.credentials.is_empty());
    // A separate process cannot bypass the installation-wide custody fence.
    let gate = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(base.join("private/.remotes-owner.lock"))
        .unwrap();
    gate.try_lock().unwrap();
    assert_eq!(
        client
            .repository_remote_settings()
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Busy
    );
    drop(gate);
    assert!(
        client
            .repository_remote_settings()
            .await
            .unwrap()
            .settings
            .credentials
            .is_empty()
    );
    let epoch_path = base.join("private/.credential-epoch.json");
    let epoch = fs::read(&epoch_path).unwrap();
    fs::write(&epoch_path, b"0").unwrap();
    assert_eq!(
        client
            .repository_remote_settings()
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Io
    );
    fs::write(&epoch_path, epoch).unwrap();
    assert!(client.repository_remote_settings().await.is_ok());
    for entry in fs::read_dir(base.join("private")).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            assert!(
                !String::from_utf8_lossy(&fs::read(path).unwrap()).contains("private-test-token")
            );
        }
    }
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepted_credential_write_survives_cancelled_waiter_and_detach() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    for name in ["private", "notes"] {
        fs::create_dir(base.join(name)).unwrap();
        fs::set_permissions(base.join(name), fs::Permissions::from_mode(0o700)).unwrap();
    }
    let custody = Arc::new(Custody(
        Mutex::new(RemoteSettings {
            host: RemoteHost::Server,
            storage: RemoteStorageState::Available,
            ssh_available: false,
            device_sign_in_available: false,
            authorization: None,
            credentials: vec![],
            targets: vec![],
            automatic_sync: false,
        }),
        AtomicBool::new(false),
    ));
    let (entered, waiting) = std::sync::mpsc::channel();
    let provider = Arc::new(Paused {
        custody: custody.clone(),
        entered,
        resume: tokio::sync::Notify::new(),
    });
    let runtime = Arc::new(
        Runtime::default()
            .with_remotes(RemoteStorage::new(&base.join("private"), provider.clone()).unwrap()),
    );
    let vault = runtime.admit(Uuid::new_v4(), &base.join("notes")).unwrap();
    let client = Arc::new(vault.attach().unwrap());
    let current = client.repository_remote_settings().await.unwrap();
    let caller = client.clone();
    let accepted = tokio::spawn(async move {
        caller
            .control_repository_remotes(RepositoryRemotesCommand::SaveCredential {
                expected_revision: current.revision,
                input: RemoteCredentialInput {
                    label: "Accepted write".into(),
                    host: "example.invalid".into(),
                    username: "fixture".into(),
                    token: "accepted-test-token".into(),
                    provider: RemoteCredentialProvider::Https,
                    replace: None,
                },
            })
            .await
    });
    tokio::task::spawn_blocking(move || waiting.recv_timeout(std::time::Duration::from_secs(10)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<u64>(
            &fs::read(base.join("private/.credential-epoch.json")).unwrap()
        )
        .unwrap(),
        1
    );
    accepted.abort();
    assert!(accepted.await.unwrap_err().is_cancelled());
    client.detach().unwrap();
    provider.resume.notify_one();
    let next = vault.attach().unwrap();
    let saved = next.repository_remote_settings().await.unwrap();
    assert_eq!(saved.settings.credentials.len(), 1);
    assert_eq!(saved.settings.credentials[0].label, "Accepted write");
    assert_eq!(custody.0.lock().unwrap().credentials.len(), 1);
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
