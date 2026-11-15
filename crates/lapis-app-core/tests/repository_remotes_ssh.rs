//! Real transport acceptance, launched only by the disposable SSH harness.
use lapis_app_core::services::{Client, RemoteCredentials, RemoteStorage, Runtime};
use lapis_app_protocol::{ErrorCode, repository_remotes::*};
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
fn jj(root: &Path, args: &[&str]) -> String {
    let output = Command::new("jj")
        .current_dir(root)
        .args([
            "--no-pager",
            "--config",
            "user.name='Core SSH fixture'",
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
    String::from_utf8(output.stdout).unwrap().trim().into()
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
fn original(snapshot: &RepositoryRemotesSnapshot) -> &RemoteRepositorySettings {
    snapshot
        .settings
        .targets
        .iter()
        .find(|t| t.target == RemoteRepositoryTarget::Original)
        .unwrap()
}
fn runtime(base: &Path) -> Arc<Runtime> {
    Arc::new(
        Runtime::default().with_remotes(
            RemoteStorage::new(&base.join("core-private"), Arc::new(Custody))
                .unwrap()
                .with_transfers(true)
                .with_known_hosts(&base.join("known 'hosts'"))
                .unwrap(),
        ),
    )
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
            target: RemoteRepositoryTarget::Original,
        })
        .await
        .unwrap();
    original(&next).review.clone().unwrap()
}
async fn push(
    client: &Client,
    review: &RemotePushReview,
) -> lapis_app_core::services::Result<RepositoryRemotesSnapshot> {
    let current = snapshot(client).await;
    client
        .control_repository_remotes(RepositoryRemotesCommand::Push {
            expected_revision: current.revision,
            target: RemoteRepositoryTarget::Original,
            review_id: review.id.clone(),
        })
        .await
}
fn receives(base: &Path) -> usize {
    fs::read_to_string(base.join("receive-count"))
        .unwrap_or_default()
        .lines()
        .count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires scripts/test-repository-remotes-ssh.mjs and disposable sshd/agent"]
async fn original_transport_reviews_one_bookmark_persists_and_recovers_without_replay() {
    let base = PathBuf::from(std::env::var_os("LAPIS_REMOTES_SSH_FIXTURE").unwrap())
        .canonicalize()
        .unwrap();
    let endpoint = std::env::var("LAPIS_REMOTES_SSH_ENDPOINT").unwrap();
    let root = base.join("core-notes");
    fs::create_dir(&root).unwrap();
    fs::create_dir(base.join("core-private")).unwrap();
    fs::set_permissions(base.join("core-private"), fs::Permissions::from_mode(0o700)).unwrap();
    jj(&root, &["git", "init", "--no-colocate", "."]);
    fs::write(root.join("Note.md"), "first reviewed version\n").unwrap();
    jj(&root, &["commit", "-m", "First selected publication"]);
    jj(&root, &["bookmark", "create", "chosen", "-r", "@-"]);
    jj(&root, &["bookmark", "create", "never-publish", "-r", "@-"]);
    let first = jj(
        &root,
        &["log", "-r", "chosen", "--no-graph", "-T", "commit_id"],
    );
    fs::write(root.join("Unsnapshotted.md"), "saved, never captured\n").unwrap();
    let checkout = fs::read(root.join(".jj/working_copy/checkout")).unwrap();
    let tree = fs::read(root.join(".jj/working_copy/tree_state")).unwrap();
    let id = Uuid::new_v4();
    let owner = runtime(&base);
    let vault = owner.admit(id, &root).unwrap();
    let client = vault.attach().unwrap();
    let initial = snapshot(&client).await;
    let saved = client
        .control_repository_remotes(RepositoryRemotesCommand::SaveConnection {
            expected_revision: initial.revision,
            target: RemoteRepositoryTarget::Original,
            connection: RemoteConnection {
                name: "chosen-remote".into(),
                url: endpoint.clone(),
                branch: "selected-core".into(),
                bookmark: "chosen".into(),
                authentication: RemoteAuthentication::SshAgent,
                credential: String::new(),
            },
        })
        .await
        .unwrap();
    assert!(!saved.settings.automatic_sync);
    assert!(original(&saved).can_fetch && original(&saved).can_push);
    assert_eq!(receives(&base), 0, "save cannot publish");
    let tested = client
        .control_repository_remotes(RepositoryRemotesCommand::TestConnection {
            expected_revision: saved.revision,
            target: RemoteRepositoryTarget::Original,
        })
        .await
        .unwrap();
    assert!(original(&tested).observation.is_none(), "test is not fetch");
    assert!(
        !remote(&base, &["show-ref", "--verify", "refs/heads/selected-core"])
            .status
            .success()
    );
    let first_review = review(&client).await;
    assert_eq!(first_review.commit, first);
    assert_eq!(first_review.outgoing, 1);
    assert_eq!(receives(&base), 0, "review cannot publish");
    client.detach().unwrap();
    close(owner).await;
    let owner = runtime(&base);
    let vault = owner.admit(id, &root).unwrap();
    let client = vault.attach().unwrap();
    assert_eq!(
        original(&snapshot(&client).await)
            .review
            .as_ref()
            .unwrap()
            .id,
        first_review.id
    );
    push(&client, &first_review).await.unwrap();
    assert_eq!(
        String::from_utf8(remote(&base, &["rev-parse", "refs/heads/selected-core"]).stdout)
            .unwrap()
            .trim(),
        first
    );
    assert!(
        !remote(&base, &["show-ref", "--verify", "refs/heads/never-publish"])
            .status
            .success()
    );
    assert!(
        !remote(
            &base,
            &["cat-file", "-e", &format!("{first}:Unsnapshotted.md")]
        )
        .status
        .success()
    );
    let sent = receives(&base);
    assert_eq!(
        push(&client, &first_review).await.unwrap_err().code(),
        ErrorCode::Conflict
    );
    assert_eq!(receives(&base), sent, "consumed reviews never replay");
    let current = snapshot(&client).await;
    let fetched = client
        .control_repository_remotes(RepositoryRemotesCommand::Fetch {
            expected_revision: current.revision,
            target: RemoteRepositoryTarget::Original,
        })
        .await
        .unwrap();
    let observation = original(&fetched).observation.as_ref().unwrap();
    assert_eq!((observation.incoming, observation.outgoing), (0, 0));
    assert_eq!(
        fs::read(root.join(".jj/working_copy/checkout")).unwrap(),
        checkout
    );
    assert_eq!(
        fs::read(root.join(".jj/working_copy/tree_state")).unwrap(),
        tree
    );
    assert_eq!(
        fs::read_to_string(root.join("Unsnapshotted.md")).unwrap(),
        "saved, never captured\n"
    );
    // Metadata-only fetch must not make the existing physical status unavailable.
    let status: serde_json::Value =
        serde_json::from_str(&client.repository_working_copy().await.unwrap()).unwrap();
    assert_eq!(status["source"], "original");
    assert_eq!(status["files"], 1);

    fs::remove_file(root.join("Unsnapshotted.md")).unwrap();
    fs::write(root.join("Note.md"), "second reviewed version\n").unwrap();
    jj(&root, &["commit", "-m", "Second selected publication"]);
    jj(&root, &["bookmark", "set", "chosen", "-r", "@-"]);
    fs::write(root.join("Unsnapshotted.md"), "still never captured\n").unwrap();
    let second_review = review(&client).await;
    fs::write(base.join("drop-next-response"), "fixture").unwrap();
    let response = push(&client, &second_review).await;
    assert!(response.is_err() || original(&response.unwrap()).publication.is_some());
    let unknown = snapshot(&client).await;
    let publication = original(&unknown).publication.as_ref().unwrap();
    assert_eq!(publication.id, second_review.id);
    assert!(!original(&unknown).can_push && !original(&unknown).can_fetch);
    assert!(original(&unknown).review.is_none());
    // Losing local binding metadata cannot hide an actual uncertain push.
    let private = base.join("core-private").join(id.to_string());
    let binding_record = private.join("original-transport/vault.json");
    let record = fs::read(&binding_record).unwrap();
    let selection = fs::read(private.join("repository-remotes.json")).unwrap();
    fs::remove_file(&binding_record).unwrap();
    assert!(
        client.repository_remote_settings().await.is_err(),
        "orphaned native publication must fail closed"
    );
    assert!(
        client
            .control_repository_remotes(RepositoryRemotesCommand::SaveConnection {
                expected_revision: unknown.revision,
                target: RemoteRepositoryTarget::Original,
                connection: original(&unknown).connection.clone().unwrap(),
            })
            .await
            .is_err(),
        "missing binding cannot permit a pending connection to rebind"
    );
    assert_eq!(
        fs::read(private.join("repository-remotes.json")).unwrap(),
        selection
    );
    fs::write(binding_record, record).unwrap();
    let sent = receives(&base);
    client.detach().unwrap();
    close(owner).await;
    let owner = runtime(&base);
    let vault = owner.admit(id, &root).unwrap();
    let client = vault.attach().unwrap();
    let current = snapshot(&client).await;
    assert_eq!(
        original(&current).publication.as_ref().unwrap().id,
        second_review.id
    );
    let recovered = client
        .control_repository_remotes(RepositoryRemotesCommand::ObservePush {
            expected_revision: current.revision,
            target: RemoteRepositoryTarget::Original,
            request_id: second_review.id.clone(),
        })
        .await
        .unwrap();
    assert!(original(&recovered).publication.is_none());
    assert_eq!(
        receives(&base),
        sent,
        "recovery observes, never republishes"
    );
    assert_eq!(
        String::from_utf8(remote(&base, &["rev-parse", "refs/heads/selected-core"]).stdout)
            .unwrap()
            .trim(),
        second_review.commit
    );
    assert!(
        !remote(
            &base,
            &[
                "cat-file",
                "-e",
                &format!("{}:Unsnapshotted.md", second_review.commit)
            ]
        )
        .status
        .success()
    );
    close(owner).await;
}
