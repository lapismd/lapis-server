//! Actual opted-in Core synchronization; only disposable fixture SSH is admitted.
use lapis_app_core::services::{BackupStorage, Client, RemoteCredentials, RemoteStorage, Runtime};
use lapis_app_protocol::{repository_remotes::*, repository_sync::*};
use std::{
    fs,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::Duration,
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
    Arc::new(
        Runtime::default()
            .with_backup(BackupStorage::new(&base.join("sync-history")).unwrap())
            .with_remotes(
                RemoteStorage::new(&base.join("sync-private"), Arc::new(Custody))
                    .unwrap()
                    .with_transfers(true)
                    .with_known_hosts(&base.join("known 'hosts'"))
                    .unwrap(),
            ),
    )
}
fn receives(base: &Path) -> usize {
    fs::read_to_string(base.join("receive-count"))
        .unwrap_or_default()
        .lines()
        .count()
}
fn remote(base: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .arg("--git-dir")
        .arg(base.join("remote.git"))
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(result.status.success(), "fixture Git observation failed");
    String::from_utf8(result.stdout).unwrap().trim().to_owned()
}
async fn wait(
    client: &Client,
    check: impl Fn(&RepositorySyncSnapshot) -> bool,
) -> RepositorySyncSnapshot {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let status = client.repository_synchronization().unwrap();
            if check(&status) {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("native synchronization did not reach the expected state")
}
async fn automatic(client: &Client, enabled: bool) -> RepositoryRemotesSnapshot {
    let current = client.repository_remote_settings().await.unwrap();
    client
        .control_repository_remotes(RepositoryRemotesCommand::SetAutomaticSync {
            expected_revision: current.revision,
            enabled,
        })
        .await
        .unwrap()
}
async fn connect(client: &Client) {
    let current = client.repository_remote_settings().await.unwrap();
    let saved = client
        .control_repository_remotes(RepositoryRemotesCommand::SaveConnection {
            expected_revision: current.revision,
            target: RemoteRepositoryTarget::Backup,
            connection: RemoteConnection {
                name: "backup".into(),
                url: std::env::var("LAPIS_REMOTES_SSH_ENDPOINT").unwrap(),
                branch: "synchronized-backup".into(),
                bookmark: "lapis-backup".into(),
                authentication: RemoteAuthentication::SshAgent,
                credential: String::new(),
            },
        })
        .await
        .unwrap();
    assert!(!saved.settings.automatic_sync);
}
async fn close(runtime: Arc<Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
async fn held(base: &Path) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !base.join("observation-held").is_file() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("fixture did not hold the real network observation");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires scripts/test-repository-remotes-ssh.mjs --sync-only"]
async fn native_sync_survives_detach_merges_then_reviews_conflicts_without_a_renderer_scheduler() {
    let base = PathBuf::from(
        std::env::var_os("LAPIS_REMOTES_SSH_FIXTURE").expect("disposable fixture required"),
    );
    for name in ["sync-first", "sync-second", "sync-history", "sync-private"] {
        let path = base.join(name);
        fs::create_dir(&path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let first_root = base.join("sync-first");
    let second_root = base.join("sync-second");
    let original_branches = remote(
        &base,
        &[
            "for-each-ref",
            "--format=%(refname):%(objectname)",
            "refs/heads",
        ],
    );
    fs::write(first_root.join("Note.md"), "shared base\n").unwrap();
    fs::write(second_root.join("Note.md"), "shared base\n").unwrap();
    let owner = runtime(&base);
    let first_id = Uuid::new_v4();
    let first_vault = owner.admit(first_id, &first_root).unwrap();
    let first = first_vault.attach().unwrap();
    first.start_backup().await.unwrap();
    let before = receives(&base);
    connect(&first).await;
    assert_eq!(
        receives(&base),
        before,
        "saving stays manual and performs no publication"
    );
    assert_eq!(first_vault.status().unwrap().work, 0);
    fs::write(base.join("hold-next-observation"), "fixture").unwrap();
    automatic(&first, true).await;
    wait(&first, |s| s.busy).await;
    held(&base).await;
    // This read must return during the held network turn, without waiting for it.
    let observer = first_vault.attach().unwrap();
    first.detach().unwrap();
    assert!(observer.repository_synchronization().unwrap().busy);
    assert!(first.repository_synchronization().is_err());
    fs::remove_file(base.join("hold-next-observation")).unwrap();
    let initial = wait(&observer, |s| !s.busy && s.last_success.is_some()).await;
    automatic(&observer, false).await;
    assert_eq!(
        first_vault.status().unwrap().work,
        0,
        "idle scheduler holds no work lease"
    );
    assert_eq!(
        remote(&base, &["show", "synchronized-backup:Note.md"]),
        "shared base"
    );
    let second_id = Uuid::new_v4();
    let second_vault = owner.admit(second_id, &second_root).unwrap();
    let second = second_vault.attach().unwrap();
    second.start_backup().await.unwrap();
    connect(&second).await;
    automatic(&second, true).await;
    wait(&second, |s| !s.busy && s.last_success.is_some()).await;
    automatic(&second, false).await;

    fs::write(first_root.join("Note.md"), "remote edit\n").unwrap();
    automatic(&observer, true).await;
    wait(&observer, |s| {
        !s.busy && s.last_success > initial.last_success
    })
    .await;
    automatic(&observer, false).await;
    assert_eq!(
        remote(&base, &["show", "synchronized-backup:Note.md"]),
        "remote edit"
    );
    fs::write(
        second_root.join("Local.md"),
        "independent saved local work\n",
    )
    .unwrap();
    let previous = second.repository_synchronization().unwrap().last_success;
    automatic(&second, true).await;
    wait(&second, |s| !s.busy && s.last_success > previous).await;
    automatic(&second, false).await;
    assert_eq!(
        fs::read_to_string(second_root.join("Note.md")).unwrap(),
        "remote edit\n"
    );
    assert_eq!(
        fs::read_to_string(second_root.join("Local.md")).unwrap(),
        "independent saved local work\n"
    );
    assert_eq!(
        remote(&base, &["show", "synchronized-backup:Local.md"]),
        "independent saved local work"
    );

    fs::write(first_root.join("Note.md"), "first conflicting edit\n").unwrap();
    let previous = observer.repository_synchronization().unwrap().last_success;
    automatic(&observer, true).await;
    wait(&observer, |s| !s.busy && s.last_success > previous).await;
    automatic(&observer, false).await;
    fs::write(second_root.join("Note.md"), "second conflicting edit\n").unwrap();
    automatic(&second, true).await;
    let conflict = wait(&second, |s| {
        !s.busy && s.phase == RepositorySyncPhase::Conflicts
    })
    .await;
    let proposal = conflict.proposal.as_ref().unwrap();
    assert_eq!(proposal.conflicts, ["Note.md"]);
    assert_eq!(
        fs::read_to_string(second_root.join("Note.md")).unwrap(),
        "second conflicting edit\n"
    );
    let candidate = proposal.candidate.as_ref().unwrap().commit.clone();
    let inspected = second
        .control_repository_synchronization(RepositorySyncCommand::Inspect {
            expected_revision: conflict.revision,
            candidate: candidate.clone(),
            path: "Note.md".into(),
        })
        .await
        .unwrap();
    assert!(
        inspected
            .conflict
            .as_ref()
            .unwrap()
            .terms
            .iter()
            .any(|t| t.text.as_deref() == Some("first conflicting edit\n"))
    );
    let resolved = second
        .control_repository_synchronization(RepositorySyncCommand::Resolve {
            expected_revision: inspected.revision,
            candidate,
            path: "Note.md".into(),
            resolution: RepositorySyncResolution::Text {
                text: "reviewed resolution\n".into(),
                executable: false,
            },
        })
        .await
        .unwrap();
    let applied = second
        .control_repository_synchronization(RepositorySyncCommand::Run {
            expected_revision: resolved.revision,
        })
        .await
        .unwrap();
    assert!(!applied.recovery);
    automatic(&second, false).await;
    assert_eq!(
        fs::read_to_string(second_root.join("Note.md")).unwrap(),
        "reviewed resolution\n"
    );
    assert_eq!(
        remote(&base, &["show", "synchronized-backup:Note.md"]),
        "reviewed resolution"
    );
    let branches = remote(
        &base,
        &[
            "for-each-ref",
            "--format=%(refname):%(objectname)",
            "refs/heads",
        ],
    );
    let unchanged = branches
        .lines()
        .filter(|b| !b.starts_with("refs/heads/synchronized-backup:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        unchanged, original_branches,
        "only the selected branch may change"
    );

    // Native dirty-editor protection retains the exact apply identity. Closing
    // refuses without replacing the live owner; restart never replays it.
    let editor = observer.open_editor("Note.md").unwrap();
    observer.mark_dirty(editor, 1).unwrap();
    automatic(&observer, true).await;
    let blocked = wait(&observer, |s| !s.busy && s.recovery).await;
    let request_id = blocked.pending_apply.clone().unwrap();
    assert_eq!(
        fs::read_to_string(first_root.join("Note.md")).unwrap(),
        "first conflicting edit\n"
    );
    let refused = owner.clone();
    assert!(
        tokio::task::spawn_blocking(move || refused.close_vault(first_id))
            .await
            .unwrap()
            .is_err()
    );
    assert!(observer.repository_synchronization().unwrap().recovery);
    observer.discard_editor(editor).unwrap();
    let closing = owner.clone();
    tokio::task::spawn_blocking(move || closing.close_vault(first_id))
        .await
        .unwrap()
        .unwrap();
    let resumed_vault = owner.admit(first_id, &first_root).unwrap();
    let resumed = resumed_vault.attach().unwrap();
    let count = receives(&base);
    let restored = wait(&resumed, |s| s.recovery).await;
    assert_eq!(restored.pending_apply.as_deref(), Some(request_id.as_str()));
    assert_eq!(
        receives(&base),
        count,
        "uncertain apply is not automatically replayed or published after restart"
    );
    assert_eq!(
        fs::read_to_string(first_root.join("Note.md")).unwrap(),
        "first conflicting edit\n"
    );
    assert!(
        resumed
            .control_repository_synchronization(RepositorySyncCommand::ResumeApply {
                expected_revision: restored.revision,
                request_id: "another-request".into(),
            })
            .await
            .is_err()
    );
    resumed
        .control_repository_synchronization(RepositorySyncCommand::ResumeApply {
            expected_revision: restored.revision,
            request_id,
        })
        .await
        .unwrap();
    automatic(&resumed, false).await;
    assert_eq!(
        fs::read_to_string(first_root.join("Note.md")).unwrap(),
        "reviewed resolution\n"
    );

    fs::write(
        first_root.join("AfterRecovery.md"),
        "one retained publication\n",
    )
    .unwrap();
    fs::write(base.join("drop-next-response"), "fixture").unwrap();
    automatic(&resumed, true).await;
    wait(&resumed, |s| {
        !s.busy && s.phase == RepositorySyncPhase::Attention
    })
    .await;
    let unknown = resumed.repository_remote_settings().await.unwrap();
    assert!(
        unknown
            .settings
            .targets
            .iter()
            .any(|t| t.target == RemoteRepositoryTarget::Backup && t.publication.is_some())
    );
    automatic(&resumed, false).await;
    let sent = receives(&base);
    resumed.detach().unwrap();
    let closing = owner.clone();
    tokio::task::spawn_blocking(move || closing.close_vault(first_id))
        .await
        .unwrap()
        .unwrap();
    let reconciled_vault = owner.admit(first_id, &first_root).unwrap();
    let reconciled = reconciled_vault.attach().unwrap();
    automatic(&reconciled, true).await;
    wait(&reconciled, |s| s.busy).await;
    wait(&reconciled, |s| {
        !s.busy && s.phase == RepositorySyncPhase::Ready
    })
    .await;
    automatic(&reconciled, false).await;
    assert_eq!(
        receives(&base),
        sent,
        "restart observes the retained push without another receive"
    );
    assert!(
        reconciled
            .repository_remote_settings()
            .await
            .unwrap()
            .settings
            .targets
            .iter()
            .all(|t| t.publication.is_none())
    );
    fs::write(first_root.join("Final.md"), "accepted before closing\n").unwrap();
    fs::write(base.join("hold-next-observation"), "fixture").unwrap();
    automatic(&reconciled, true).await;
    held(&base).await;
    reconciled.detach().unwrap();
    let closing = owner.clone();
    let drain = tokio::task::spawn_blocking(move || closing.close_vault(first_id));
    tokio::time::timeout(Duration::from_secs(5), async {
        while reconciled_vault.status().unwrap().phase == lapis_app_protocol::VaultPhase::Open {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        !drain.is_finished(),
        "close must join the accepted network cycle"
    );
    fs::remove_file(base.join("hold-next-observation")).unwrap();
    drain.await.unwrap().unwrap();
    assert_eq!(
        reconciled_vault.status().unwrap().phase,
        lapis_app_protocol::VaultPhase::Closed
    );
    assert_eq!(
        remote(&base, &["show", "synchronized-backup:Final.md"]),
        "accepted before closing"
    );
    second.detach().unwrap();
    close(owner).await;

    let restarted = runtime(&base);
    let reopened = restarted.admit(second_id, &second_root).unwrap();
    let cold = reopened.attach().unwrap();
    assert!(
        !cold
            .repository_remote_settings()
            .await
            .unwrap()
            .settings
            .automatic_sync
    );
    let count = receives(&base);
    tokio::time::sleep(Duration::from_secs(6)).await;
    assert_eq!(
        receives(&base),
        count,
        "restart preserves the saved manual preference"
    );
    assert_eq!(reopened.status().unwrap().work, 0);
    close(restarted).await;
}
