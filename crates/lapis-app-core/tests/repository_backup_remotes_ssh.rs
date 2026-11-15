//! Shared backup transfers against the real disposable Git/SSH endpoint.
use lapis_app_core::services::{BackupStorage, Client, RemoteCredentials, RemoteStorage, Runtime};
use lapis_app_protocol::{repository_backup::RepositoryBackupCommand, repository_remotes::*};
use std::{
    fs,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
use uuid::Uuid;

struct Custody;
impl RemoteCredentials for Custody {
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
fn runtime(base: &Path) -> Arc<Runtime> {
    runtime_with_transfers(base, true)
}
fn runtime_with_transfers(base: &Path, transfers: bool) -> Arc<Runtime> {
    Arc::new(
        Runtime::default()
            .with_backup(BackupStorage::new(&base.join("backup-history")).unwrap())
            .with_remotes(
                RemoteStorage::new(&base.join("backup-connections"), Arc::new(Custody))
                    .unwrap()
                    .with_transfers(transfers)
                    .with_known_hosts(&base.join("known 'hosts'"))
                    .unwrap(),
            ),
    )
}
fn backup(snapshot: &RepositoryRemotesSnapshot) -> &RemoteRepositorySettings {
    snapshot
        .settings
        .targets
        .iter()
        .find(|t| t.target == RemoteRepositoryTarget::Backup)
        .unwrap()
}
fn receives(base: &Path) -> usize {
    fs::read_to_string(base.join("receive-count"))
        .unwrap_or_default()
        .lines()
        .count()
}
fn remote(base: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .arg("--git-dir")
        .arg(base.join("remote.git"))
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap()
}
async fn close(runtime: Arc<Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
async fn snapshot(client: &Client) -> RepositoryRemotesSnapshot {
    client.repository_remote_settings().await.unwrap()
}
async fn review(client: &Client) -> RemotePushReview {
    let current = snapshot(client).await;
    let next = client
        .control_repository_remotes(RepositoryRemotesCommand::ReviewPush {
            expected_revision: current.revision,
            target: RemoteRepositoryTarget::Backup,
        })
        .await
        .unwrap();
    backup(&next).review.clone().unwrap()
}
async fn push(
    client: &Client,
    review: &RemotePushReview,
) -> lapis_app_core::services::Result<RepositoryRemotesSnapshot> {
    let current = snapshot(client).await;
    client
        .control_repository_remotes(RepositoryRemotesCommand::Push {
            expected_revision: current.revision,
            target: RemoteRepositoryTarget::Backup,
            review_id: review.id.clone(),
        })
        .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires scripts/test-repository-remotes-ssh.mjs and disposable sshd/agent"]
async fn backup_reviews_recorded_checkpoints_and_recovers_without_automatic_publication() {
    let base = PathBuf::from(std::env::var_os("LAPIS_REMOTES_SSH_FIXTURE").unwrap())
        .canonicalize()
        .unwrap();
    let root = base.join("backup-notes");
    for path in [
        &root,
        &base.join("backup-history"),
        &base.join("backup-connections"),
    ] {
        fs::create_dir(path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fs::write(root.join("Note.md"), "first recorded checkpoint\n").unwrap();
    let before = receives(&base);
    let id = Uuid::new_v4();
    let owner = runtime(&base);
    let vault = owner.admit(id, &root).unwrap();
    let client = vault.attach().unwrap();
    let status = client.start_backup().await.unwrap();
    let checkpoint = status.borrow().checkpoint.as_ref().unwrap().commit.clone();
    assert!(!root.join(".jj").exists());
    let initial = snapshot(&client).await;
    let saved = client
        .control_repository_remotes(RepositoryRemotesCommand::SaveConnection {
            expected_revision: initial.revision,
            target: RemoteRepositoryTarget::Backup,
            connection: RemoteConnection {
                name: "backup".into(),
                url: std::env::var("LAPIS_REMOTES_SSH_ENDPOINT").unwrap(),
                branch: "selected-backup".into(),
                bookmark: String::new(),
                authentication: RemoteAuthentication::SshAgent,
                credential: String::new(),
            },
        })
        .await
        .unwrap();
    assert_eq!(saved.capabilities.targets, [RemoteRepositoryTarget::Backup]);
    assert!(!saved.settings.automatic_sync);
    assert_eq!(receives(&base), before, "saving never publishes");
    assert!(backup(&saved).can_fetch && backup(&saved).can_push);
    client
        .control_repository_remotes(RepositoryRemotesCommand::TestConnection {
            expected_revision: saved.revision,
            target: RemoteRepositoryTarget::Backup,
        })
        .await
        .unwrap();
    let current = snapshot(&client).await;
    let fetched = client
        .control_repository_remotes(RepositoryRemotesCommand::Fetch {
            expected_revision: current.revision,
            target: RemoteRepositoryTarget::Backup,
        })
        .await
        .unwrap();
    let counts = backup(&fetched).observation.as_ref().unwrap();
    assert_eq!((counts.incoming, counts.outgoing), (0, 1));
    let first_review = review(&client).await;
    assert_eq!(first_review.commit, checkpoint);
    fs::write(root.join("Note.md"), "later saved checkpoint\n").unwrap();
    fs::write(root.join("Later.md"), "not part of the first review\n").unwrap();
    let captured = client
        .control_repository_backup(RepositoryBackupCommand::Capture {})
        .await
        .unwrap();
    assert_ne!(
        captured.status.checkpoint.as_ref().unwrap().commit,
        first_review.commit
    );
    assert_eq!(
        receives(&base),
        before,
        "saving a later checkpoint stays local"
    );
    push(&client, &first_review).await.unwrap();
    assert_eq!(receives(&base), before + 1);
    assert_eq!(
        String::from_utf8(remote(&base, &["rev-parse", "refs/heads/selected-backup"]).stdout)
            .unwrap()
            .trim(),
        first_review.commit
    );
    assert!(
        !remote(
            &base,
            &[
                "cat-file",
                "-e",
                &format!("{}:Later.md", first_review.commit)
            ]
        )
        .status
        .success()
    );
    assert_eq!(
        fs::read_to_string(root.join("Later.md")).unwrap(),
        "not part of the first review\n"
    );
    let second_review = review(&client).await;
    assert_ne!(second_review.commit, first_review.commit);
    fs::write(base.join("drop-next-response"), "fixture").unwrap();
    let outcome = push(&client, &second_review).await;
    assert!(outcome.is_err() || backup(&outcome.unwrap()).publication.is_some());
    let uncertain = snapshot(&client).await;
    assert_eq!(
        backup(&uncertain).publication.as_ref().unwrap().id,
        second_review.id
    );
    let sent = receives(&base);
    client.detach().unwrap();
    drop(status);
    close(owner).await;
    let owner = runtime(&base);
    let vault = owner.admit(id, &root).unwrap();
    let client = vault.attach().unwrap();
    let current = snapshot(&client).await;
    assert_eq!(
        backup(&current).publication.as_ref().unwrap().id,
        second_review.id
    );
    let observed = client
        .control_repository_remotes(RepositoryRemotesCommand::ObservePush {
            expected_revision: current.revision,
            target: RemoteRepositoryTarget::Backup,
            request_id: second_review.id.clone(),
        })
        .await
        .unwrap();
    assert!(backup(&observed).publication.is_none());
    assert!(!observed.settings.automatic_sync);
    assert_eq!(
        receives(&base),
        sent,
        "restart recovery only observes the original request"
    );
    assert_eq!(
        String::from_utf8(remote(&base, &["rev-parse", "refs/heads/selected-backup"]).stdout)
            .unwrap()
            .trim(),
        second_review.commit
    );
    // With capture stopped after restart, fetching only imports metadata.
    fs::write(
        root.join("Uncaptured.md"),
        "standby fetch must not capture this\n",
    )
    .unwrap();
    let current = snapshot(&client).await;
    let fetched = client
        .control_repository_remotes(RepositoryRemotesCommand::Fetch {
            expected_revision: current.revision,
            target: RemoteRepositoryTarget::Backup,
        })
        .await
        .unwrap();
    assert!(backup(&fetched).can_fetch && backup(&fetched).can_push);
    let standby = client
        .control_repository_backup(RepositoryBackupCommand::Status {})
        .await
        .unwrap();
    assert!(
        matches!(
            standby.status.phase,
            lapis_app_protocol::repository_backup::RepositoryBackupPhase::Failed
        ),
        "metadata fetch must not start a capture worker"
    );
    let standby_review = review(&client).await;
    assert_eq!(standby_review.commit, second_review.commit);
    assert_eq!(receives(&base), sent);

    // A second unknown outcome can be stopped locally without transfer admission.
    let status = client.start_backup().await.unwrap();
    let final_review = review(&client).await;
    assert_ne!(final_review.commit, standby_review.commit);
    fs::write(base.join("drop-next-response"), "fixture").unwrap();
    let outcome = push(&client, &final_review).await;
    assert!(outcome.is_err() || backup(&outcome.unwrap()).publication.is_some());
    let sent = receives(&base);
    client.detach().unwrap();
    drop(status);
    close(owner).await;
    let owner = runtime_with_transfers(&base, false);
    let vault = owner.admit(id, &root).unwrap();
    let client = vault.attach().unwrap();
    let current = snapshot(&client).await;
    assert_eq!(
        backup(&current).publication.as_ref().unwrap().id,
        final_review.id
    );
    assert!(!backup(&current).can_fetch && !backup(&current).can_push);
    let stopped = client
        .control_repository_remotes(RepositoryRemotesCommand::StopPush {
            expected_revision: current.revision,
            target: RemoteRepositoryTarget::Backup,
            request_id: final_review.id.clone(),
        })
        .await
        .unwrap();
    assert!(backup(&stopped).publication.is_none());
    assert_eq!(
        push(&client, &final_review).await.unwrap_err().code(),
        lapis_app_protocol::ErrorCode::Conflict,
        "a stopped publication does not revive its consumed review"
    );
    assert_eq!(receives(&base), sent, "local stop never publishes");
    assert_eq!(
        fs::read_to_string(root.join("Uncaptured.md")).unwrap(),
        "standby fetch must not capture this\n"
    );
    assert!(!root.join(".jj").exists());
    close(owner).await;
}
