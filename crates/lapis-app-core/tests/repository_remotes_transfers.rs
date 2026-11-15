use lapis_app_core::services::{BackupStorage, RemoteCredentials, RemoteStorage, Runtime};
use lapis_app_protocol::{ErrorCode, repository_remotes::*};
use std::{fs, os::unix::fs::PermissionsExt as _, sync::Arc};
use std::{
    path::Path,
    process::Command,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use uuid::Uuid;

struct Locked;
impl RemoteCredentials for Locked {
    fn inventory(&self) -> lapis_app_core::services::Result<RemoteSettings> {
        Ok(RemoteSettings {
            host: RemoteHost::Server,
            storage: RemoteStorageState::Locked,
            ssh_available: true,
            device_sign_in_available: false,
            authorization: None,
            credentials: vec![],
            targets: vec![],
            automatic_sync: false,
        })
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unconfigured_transfer_refuses_before_creating_transport() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir(root.join("notes")).unwrap();
    fs::create_dir(root.join("private")).unwrap();
    fs::set_permissions(root.join("private"), fs::Permissions::from_mode(0o700)).unwrap();
    let runtime = Arc::new(
        Runtime::default()
            .with_remotes(RemoteStorage::new(&root.join("private"), Arc::new(Locked)).unwrap()),
    );
    let vault = runtime.admit(Uuid::new_v4(), &root.join("notes")).unwrap();
    let client = vault.attach().unwrap();
    let error = client
        .control_repository_remotes(RepositoryRemotesCommand::TestConnection {
            expected_revision: 0,
            target: RemoteRepositoryTarget::Original,
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidRequest);
    assert_eq!(fs::read_dir(root.join("notes")).unwrap().count(), 0);
    let settings = client.repository_remote_settings().await.unwrap();
    assert_eq!(settings.revision, 0);
    assert!(
        settings
            .settings
            .targets
            .iter()
            .all(|t| !t.can_fetch && !t.can_push)
    );
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

type Pause = (mpsc::Sender<()>, mpsc::Receiver<()>);
struct TokenCustody {
    id: String,
    calls: AtomicUsize,
    pause: Mutex<Option<Pause>>,
}
impl RemoteCredentials for TokenCustody {
    fn inventory(&self) -> lapis_app_core::services::Result<RemoteSettings> {
        let mut settings = Locked.inventory()?;
        settings.storage = RemoteStorageState::Available;
        settings.credentials.push(RemoteCredentialSummary {
            id: self.id.clone(),
            label: "Fixture token".into(),
            host: "example.invalid".into(),
            username: "fixture".into(),
            method: RemoteCredentialMethod::Token,
            available: true,
        });
        Ok(settings)
    }
    fn resolve(
        &self,
        _connection: &RemoteConnection,
    ) -> lapis_app_core::services::Result<jj_vault::GitCredentials> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some((entered, resume)) = self.pause.lock().unwrap().take() {
            entered.send(()).unwrap();
            resume.recv_timeout(Duration::from_secs(20)).unwrap();
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Fixture refuses before any network access",
        )
        .into())
    }
}
fn jj(root: &Path, args: &[&str]) {
    let output = Command::new("jj")
        .current_dir(root)
        .args([
            "--no-pager",
            "--config",
            "user.name='Selection fixture'",
            "--config",
            "user.email='fixture@invalid'",
            "--config",
            "signing.behavior='drop'",
        ])
        .args(args)
        .env("JJ_CONFIG", "/dev/null")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn backup_without_capture_keeps_counts_unknown_and_refuses_invalid_ids_before_custody() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    for path in ["notes", "private", "history"] {
        fs::create_dir(base.join(path)).unwrap();
        fs::set_permissions(base.join(path), fs::Permissions::from_mode(0o700)).unwrap();
    }
    fs::write(base.join("notes/Note.md"), "never captured\n").unwrap();
    let custody = Arc::new(TokenCustody {
        id: Uuid::new_v4().to_string(),
        calls: AtomicUsize::new(0),
        pause: Mutex::new(None),
    });
    let runtime = Arc::new(
        Runtime::default()
            .with_backup(BackupStorage::new(&base.join("history")).unwrap())
            .with_remotes(
                RemoteStorage::new(&base.join("private"), custody.clone())
                    .unwrap()
                    .with_transfers(true),
            ),
    );
    let vault = runtime.admit(Uuid::new_v4(), &base.join("notes")).unwrap();
    let client = vault.attach().unwrap();
    let current = client
        .control_repository_remotes(RepositoryRemotesCommand::SaveConnection {
            expected_revision: 0,
            target: RemoteRepositoryTarget::Backup,
            connection: RemoteConnection {
                name: "backup".into(),
                url: "https://example.invalid/backup.git".into(),
                branch: "backup".into(),
                bookmark: String::new(),
                authentication: RemoteAuthentication::Token,
                credential: custody.id.clone(),
            },
        })
        .await
        .unwrap();
    let target = &current.settings.targets[0];
    assert_eq!(target.target, RemoteRepositoryTarget::Backup);
    assert!(target.can_fetch && !target.can_push);
    assert!(target.observation.is_none() && target.review.is_none());
    assert!(!current.settings.automatic_sync);
    for command in [
        RepositoryRemotesCommand::Push {
            expected_revision: current.revision,
            target: RemoteRepositoryTarget::Backup,
            review_id: "not-a-review".into(),
        },
        RepositoryRemotesCommand::ObservePush {
            expected_revision: current.revision,
            target: RemoteRepositoryTarget::Backup,
            request_id: "not-a-request".into(),
        },
    ] {
        assert_eq!(
            client
                .control_repository_remotes(command)
                .await
                .unwrap_err()
                .code(),
            ErrorCode::Conflict
        );
    }
    assert_eq!(custody.calls.load(Ordering::SeqCst), 0);
    assert_eq!(fs::read_dir(base.join("history")).unwrap().count(), 0);
    assert!(!base.join("notes/.jj").exists());
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_review_ids_never_resolve_tokens_and_accepted_resolution_holds_the_process_guard() {
    use lapis_app_core::{repository::remotes::selection::Selection, vault::VaultRoot};
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let root = base.join("notes");
    fs::create_dir(&root).unwrap();
    fs::create_dir(base.join("private")).unwrap();
    fs::set_permissions(base.join("private"), fs::Permissions::from_mode(0o700)).unwrap();
    jj(&root, &["git", "init", "--no-colocate", "."]);
    fs::write(root.join("Note.md"), "saved\n").unwrap();
    jj(&root, &["commit", "-m", "Existing original"]);
    jj(&root, &["bookmark", "create", "chosen", "-r", "@-"]);
    let custody = Arc::new(TokenCustody {
        id: Uuid::new_v4().to_string(),
        calls: AtomicUsize::new(0),
        pause: Mutex::new(None),
    });
    let runtime = Arc::new(
        Runtime::default().with_remotes(
            RemoteStorage::new(&base.join("private"), custody.clone())
                .unwrap()
                .with_transfers(true),
        ),
    );
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, &root).unwrap();
    let client = Arc::new(vault.attach().unwrap());
    let initial = client.repository_remote_settings().await.unwrap();
    let current = client
        .control_repository_remotes(RepositoryRemotesCommand::SaveConnection {
            expected_revision: initial.revision,
            target: RemoteRepositoryTarget::Original,
            connection: RemoteConnection {
                name: "selected".into(),
                url: "https://example.invalid/notes.git".into(),
                branch: "main".into(),
                bookmark: "chosen".into(),
                authentication: RemoteAuthentication::Token,
                credential: custody.id.clone(),
            },
        })
        .await
        .unwrap();
    let revision = current.revision;
    let private = base.join("private").join(id.to_string());
    let before = fs::read(private.join("repository-remotes.json")).unwrap();
    for command in [
        RepositoryRemotesCommand::Push {
            expected_revision: revision,
            target: RemoteRepositoryTarget::Original,
            review_id: "not-a-review".into(),
        },
        RepositoryRemotesCommand::ObservePush {
            expected_revision: revision,
            target: RemoteRepositoryTarget::Original,
            request_id: "not-a-request".into(),
        },
    ] {
        assert_eq!(
            client
                .control_repository_remotes(command)
                .await
                .unwrap_err()
                .code(),
            ErrorCode::Conflict
        );
    }
    assert_eq!(custody.calls.load(Ordering::SeqCst), 0);
    assert!(!private.join("original-transport").exists());
    let (entered, waiting) = mpsc::channel();
    let (resume, released) = mpsc::channel();
    *custody.pause.lock().unwrap() = Some((entered, released));
    let caller = client.clone();
    let accepted = tokio::spawn(async move {
        caller
            .control_repository_remotes(RepositoryRemotesCommand::TestConnection {
                expected_revision: revision,
                target: RemoteRepositoryTarget::Original,
            })
            .await
    });
    tokio::task::spawn_blocking(move || waiting.recv_timeout(Duration::from_secs(10)))
        .await
        .unwrap()
        .unwrap();
    let competing = Selection::open(
        VaultRoot::open(&private).unwrap(),
        &VaultRoot::open(&root).unwrap(),
    )
    .unwrap();
    assert!(
        competing
            .configure(
                revision,
                RemoteRepositoryTarget::Backup,
                RemoteConnection {
                    name: "backup".into(),
                    url: "https://other.invalid/notes.git".into(),
                    branch: "main".into(),
                    bookmark: "lapis-backup".into(),
                    authentication: RemoteAuthentication::Anonymous,
                    credential: String::new(),
                },
                || Ok(false)
            )
            .is_err(),
        "another process cannot rebind during resolution"
    );
    assert_eq!(
        fs::read(private.join("repository-remotes.json")).unwrap(),
        before
    );
    accepted.abort();
    assert!(accepted.await.unwrap_err().is_cancelled());
    client.detach().unwrap();
    resume.send(()).unwrap();
    let next = vault.attach().unwrap();
    assert_eq!(
        next.repository_remote_settings().await.unwrap().revision,
        revision
    );
    assert_eq!(custody.calls.load(Ordering::SeqCst), 1);
    assert!(
        !private.join("original-transport").exists(),
        "denied resolution initializes no transfer"
    );
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
