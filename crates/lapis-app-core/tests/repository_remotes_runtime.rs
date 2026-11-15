use lapis_app_core::services::{Client, RemoteCredentials, RemoteStorage, Runtime, VaultService};
use lapis_app_protocol::{VaultPhase, repository_remotes::*};
use std::{
    fs,
    os::unix::fs::PermissionsExt as _,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use uuid::Uuid;

type Pause = (mpsc::Sender<()>, mpsc::Receiver<()>);
#[derive(Default)]
struct Credentials {
    pause: Mutex<Option<Pause>>,
}
impl Credentials {
    fn pause(&self) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (entered, wait) = mpsc::channel();
        let (resume, released) = mpsc::channel();
        *self.pause.lock().unwrap() = Some((entered, released));
        (wait, resume)
    }
}
impl RemoteCredentials for Credentials {
    fn inventory(&self) -> lapis_app_core::services::Result<RemoteSettings> {
        if let Some((entered, released)) = self.pause.lock().unwrap().take() {
            entered.send(()).unwrap();
            released
                .recv_timeout(Duration::from_secs(20))
                .map_err(|_| std::io::Error::other("Fixture inventory did not resume"))?;
        }
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
struct Fixture {
    temp: tempfile::TempDir,
    runtime: Arc<Runtime>,
    vault: Arc<VaultService>,
    client: Arc<Client>,
    credentials: Arc<Credentials>,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::create_dir(root.join("notes")).unwrap();
        fs::create_dir(root.join("private")).unwrap();
        fs::set_permissions(root.join("private"), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(root.join("notes/Note.md"), "saved bytes").unwrap();
        let credentials = Arc::new(Credentials::default());
        let runtime =
            Arc::new(Runtime::default().with_remotes(
                RemoteStorage::new(&root.join("private"), credentials.clone()).unwrap(),
            ));
        let vault = runtime.admit(Uuid::new_v4(), &root.join("notes")).unwrap();
        let client = Arc::new(vault.attach().unwrap());
        Self {
            temp,
            runtime,
            vault,
            client,
            credentials,
        }
    }
    fn command() -> RepositoryRemotesCommand {
        RepositoryRemotesCommand::SaveConnection {
            expected_revision: 0,
            target: RemoteRepositoryTarget::Backup,
            connection: RemoteConnection {
                name: "backup".into(),
                url: "https://unreachable.invalid/notes.git".into(),
                branch: "main".into(),
                bookmark: "lapis-backup".into(),
                authentication: RemoteAuthentication::Anonymous,
                credential: String::new(),
            },
        }
    }
    async fn close(&self) {
        let runtime = self.runtime.clone();
        tokio::task::spawn_blocking(move || runtime.shutdown())
            .await
            .unwrap()
            .unwrap();
    }
}
async fn entered(receiver: mpsc::Receiver<()>) {
    tokio::task::spawn_blocking(move || receiver.recv_timeout(Duration::from_secs(10)))
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saving_a_connection_is_private_manual_and_performs_no_repository_initialization() {
    let f = Fixture::new();
    let initial = f.client.repository_remote_settings().await.unwrap();
    assert_eq!(initial.revision, 0);
    let saved = f
        .client
        .control_repository_remotes(Fixture::command())
        .await
        .unwrap();
    assert_eq!(saved.revision, 1);
    assert!(!saved.settings.automatic_sync);
    assert!(!saved.settings.targets[0].can_fetch);
    assert!(!saved.settings.targets[0].can_push);
    assert!(matches!(saved.settings.storage, RemoteStorageState::Locked));
    let root = f.temp.path().join("notes");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    assert_eq!(
        fs::read_to_string(root.join("Note.md")).unwrap(),
        "saved bytes"
    );
    assert!(
        f.client
            .control_repository_remotes(Fixture::command())
            .await
            .is_err(),
        "stale saves must refuse"
    );
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn detached_waiter_is_fenced_and_a_later_client_observes_the_accepted_write() {
    let f = Fixture::new();
    let (wait, resume) = f.credentials.pause();
    let client = f.client.clone();
    let save =
        tokio::spawn(async move { client.control_repository_remotes(Fixture::command()).await });
    entered(wait).await;
    f.client.detach().unwrap();
    let next = Arc::new(f.vault.attach().unwrap());
    let query = tokio::spawn(async move { next.repository_remote_settings().await });
    resume.send(()).unwrap();
    assert!(
        save.await.unwrap().is_err(),
        "retired callers cannot receive a late result"
    );
    let observed = query.await.unwrap().unwrap();
    assert_eq!(observed.revision, 1);
    assert_eq!(
        observed.settings.targets[0]
            .connection
            .as_ref()
            .unwrap()
            .branch,
        "main"
    );
    assert!(f.client.repository_remote_settings().await.is_err());
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_response_keeps_accepted_work_until_close_drains_then_reopens_persisted_selection()
 {
    let f = Fixture::new();
    let (wait, resume) = f.credentials.pause();
    let client = f.client.clone();
    let save =
        tokio::spawn(async move { client.control_repository_remotes(Fixture::command()).await });
    entered(wait).await;
    save.abort();
    assert!(save.await.unwrap_err().is_cancelled());
    f.client.detach().unwrap();
    let runtime = f.runtime.clone();
    let id = f.vault.id();
    let closing = tokio::task::spawn_blocking(move || runtime.close_vault(id));
    tokio::time::timeout(Duration::from_secs(5), async {
        while f.vault.status().unwrap().phase == VaultPhase::Open {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!closing.is_finished());
    assert_eq!(f.vault.status().unwrap().work, 1);
    resume.send(()).unwrap();
    closing.await.unwrap().unwrap();
    let reopened = f.runtime.admit(id, &f.temp.path().join("notes")).unwrap();
    let client = reopened.attach().unwrap();
    let saved = client.repository_remote_settings().await.unwrap();
    assert_eq!(saved.revision, 1);
    assert!(!saved.settings.automatic_sync);
    assert_eq!(reopened.status().unwrap().work, 0);
    f.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn original_discovery_is_passive_sanitized_and_never_chooses_origin() {
    use std::process::Command;
    let f = Fixture::new();
    let root = f.temp.path().join("notes");
    for args in [
        vec!["git", "init", "--no-colocate", "."],
        vec![
            "git",
            "remote",
            "add",
            "origin",
            "https://example.invalid/notes.git",
        ],
        vec![
            "git",
            "remote",
            "add",
            "unsafe",
            "https://fixture:synthetic@example.invalid/private.git",
        ],
    ] {
        let result = Command::new("jj")
            .current_dir(&root)
            .env("JJ_CONFIG", "/dev/null")
            .args([
                "--no-pager",
                "--config",
                "user.name='Fixture'",
                "--config",
                "user.email='fixture@invalid'",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(result.status.success(), "JJ fixture setup failed");
    }
    let config_path = root.join(".jj/repo/store/git/config");
    let config = fs::read(&config_path).unwrap();
    let checkout = fs::read(root.join(".jj/working_copy/checkout")).unwrap();
    let tree = fs::read(root.join(".jj/working_copy/tree_state")).unwrap();
    let initial = f.client.repository_remote_settings().await.unwrap();
    let value = serde_json::to_value(&initial).unwrap();
    let original = value["settings"]["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["target"] == "original")
        .expect("an existing original must be discoverable");
    assert!(
        original["connection"].is_null(),
        "discovery must not choose origin"
    );
    assert_eq!(original["canFetch"], false);
    assert_eq!(original["canPush"], false);
    let remotes = original["remotes"].as_array().unwrap();
    assert!(
        remotes
            .iter()
            .any(|r| r["name"] == "origin" && r["url"] == "https://example.invalid/notes.git")
    );
    assert!(
        remotes
            .iter()
            .any(|r| r["name"] == "unsafe" && r["url"].is_null())
    );
    assert!(!serde_json::to_string(&value).unwrap().contains("synthetic"));
    assert_eq!(fs::read(&config_path).unwrap(), config);
    assert_eq!(
        fs::read(root.join(".jj/working_copy/checkout")).unwrap(),
        checkout
    );
    assert_eq!(
        fs::read(root.join(".jj/working_copy/tree_state")).unwrap(),
        tree
    );
    let again = f.client.repository_remote_settings().await.unwrap();
    assert_eq!(
        again.revision, initial.revision,
        "stable passive observations retain their revision"
    );
    assert!(!again.settings.automatic_sync);
    f.close().await;
}

fn fixture_jj(root: &std::path::Path, args: &[&str]) {
    let result = std::process::Command::new("jj")
        .current_dir(root)
        .env("JJ_CONFIG", "/dev/null")
        .args([
            "--no-pager",
            "--config",
            "user.name='Fixture'",
            "--config",
            "user.email='fixture@invalid'",
            "--config",
            "signing.behavior='drop'",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(result.status.success(), "JJ fixture command failed");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn original_save_requires_the_discovered_revision_and_never_transfers() {
    let f = Fixture::new();
    let root = f.temp.path().join("notes");
    fixture_jj(&root, &["git", "init", "--no-colocate", "."]);
    fixture_jj(&root, &["commit", "-m", "Saved fixture"]);
    fixture_jj(&root, &["bookmark", "create", "main", "-r", "@-"]);
    fixture_jj(
        &root,
        &[
            "git",
            "remote",
            "add",
            "origin",
            "https://example.invalid/original.git",
        ],
    );
    fixture_jj(
        &root,
        &[
            "git",
            "remote",
            "add",
            "other",
            "https://example.invalid/other.git",
        ],
    );
    let initial = f.client.repository_remote_settings().await.unwrap();
    let connection = RemoteConnection {
        name: "origin".into(),
        url: "https://unreachable.invalid/chosen.git".into(),
        branch: "publish".into(),
        bookmark: "main".into(),
        authentication: RemoteAuthentication::Anonymous,
        credential: String::new(),
    };
    let save = |revision, connection| RepositoryRemotesCommand::SaveConnection {
        expected_revision: revision,
        target: RemoteRepositoryTarget::Original,
        connection,
    };
    fixture_jj(
        &root,
        &[
            "git",
            "remote",
            "set-url",
            "origin",
            "https://example.invalid/externally-changed.git",
        ],
    );
    let config_path = root.join(".jj/repo/store/git/config");
    let config = fs::read(&config_path).unwrap();
    let stale = f
        .client
        .control_repository_remotes(save(initial.revision, connection.clone()))
        .await
        .unwrap_err();
    assert_eq!(stale.code(), lapis_app_protocol::ErrorCode::Conflict);
    assert_eq!(
        fs::read(&config_path).unwrap(),
        config,
        "stale saves must not overwrite an external remote edit"
    );
    let discovered = f.client.repository_remote_settings().await.unwrap();
    let checkout = fs::read(root.join(".jj/working_copy/checkout")).unwrap();
    let tree = fs::read(root.join(".jj/working_copy/tree_state")).unwrap();
    let saved = f
        .client
        .control_repository_remotes(save(discovered.revision, connection.clone()))
        .await
        .unwrap();
    let original = saved
        .settings
        .targets
        .iter()
        .find(|t| t.target == RemoteRepositoryTarget::Original)
        .unwrap();
    assert_eq!(original.connection.as_ref(), Some(&connection));
    assert!(
        !original.can_fetch && !original.can_push,
        "save alone admits no network operation"
    );
    assert!(!saved.settings.automatic_sync);
    let native = fs::read_to_string(&config_path).unwrap();
    assert!(native.contains("https://unreachable.invalid/chosen.git"));
    assert!(native.contains("https://example.invalid/other.git"));
    assert_eq!(
        fs::read(root.join(".jj/working_copy/checkout")).unwrap(),
        checkout
    );
    assert_eq!(
        fs::read(root.join(".jj/working_copy/tree_state")).unwrap(),
        tree
    );
    assert!(
        !f.temp
            .path()
            .join("private")
            .join(f.vault.id().to_string())
            .join("original-transport")
            .exists()
    );
    let mut wrong = connection;
    wrong.authentication = RemoteAuthentication::Token;
    wrong.credential = Uuid::new_v4().to_string();
    let before = fs::read(&config_path).unwrap();
    assert!(
        f.client
            .control_repository_remotes(save(saved.revision, wrong))
            .await
            .is_err()
    );
    assert_eq!(
        fs::read(&config_path).unwrap(),
        before,
        "credential admission must precede Git config writes"
    );
    let mut ssh = original.connection.clone().unwrap();
    ssh.name = "host-ssh".into();
    ssh.url = "ssh://git@unreachable.invalid/notes.git".into();
    ssh.authentication = RemoteAuthentication::SshDefault;
    ssh.credential.clear();
    let saved_ssh = f
        .client
        .control_repository_remotes(save(saved.revision, ssh.clone()))
        .await
        .unwrap();
    assert_eq!(
        saved_ssh
            .settings
            .targets
            .iter()
            .find(|t| t.target == RemoteRepositoryTarget::Original)
            .unwrap()
            .connection
            .as_ref(),
        Some(&ssh)
    );
    assert!(
        matches!(saved_ssh.settings.storage, RemoteStorageState::Locked),
        "host SSH selection requires no unlocked token/forge store"
    );
    assert!(
        !f.temp
            .path()
            .join("private")
            .join(f.vault.id().to_string())
            .join("original-transport")
            .exists()
    );
    f.close().await;
}
