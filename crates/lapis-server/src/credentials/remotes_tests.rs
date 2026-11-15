use super::*;
use lapis_app_core::services::{RemoteCredentialAction as Action, RemoteCredentials};
use lapis_app_protocol::repository_remotes::*;
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt as _, sync::Arc};
struct Network;
#[async_trait::async_trait(?Send)]
impl jj_forge::accounts::http::Transport for Network {
    async fn request(
        &self,
        _url: &str,
        token: Option<&str>,
        _body: Option<Value>,
    ) -> jj_forge::Result<Value> {
        Ok(json!({"id":if token==Some("wrong-account-fixture"){99}else{42},"login":"fixture"}))
    }
}
struct Fixture {
    root: tempfile::TempDir,
    installation: Uuid,
    owner: Uuid,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("private")).unwrap();
        fs::set_permissions(
            root.path().join("private"),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        fs::write(root.path().join("key"), [7u8; 32]).unwrap();
        Self {
            root,
            installation: Uuid::new_v4(),
            owner: Uuid::new_v4(),
        }
    }
    fn store(&self) -> Arc<CredentialStore> {
        Arc::new(CredentialStore::open(
            &self.root.path().join("private"),
            self.installation,
            Some(&self.root.path().join("key")),
        ))
    }
    fn provider(
        &self,
        store: &Arc<CredentialStore>,
        owner: Uuid,
    ) -> Arc<lapis_app_core::services::NativeRemoteCredentials> {
        store
            .remote_credentials_with_transport(
                owner,
                self.root.path().join("accounts"),
                None,
                true,
                Arc::new(Network),
            )
            .unwrap()
    }
    fn input(&self, provider: RemoteCredentialProvider, host: &str) -> RemoteCredentialInput {
        RemoteCredentialInput {
            label: "Encrypted fixture".into(),
            host: host.into(),
            username: "fixture".into(),
            token: "private-token-fixture".into(),
            provider,
            replace: None,
        }
    }
    fn no_plaintext(&self) {
        fn walk(path: &Path) {
            for entry in fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path);
                } else {
                    let bytes = fs::read(&path).unwrap();
                    assert!(
                        !bytes
                            .windows(b"private-token-fixture".len())
                            .any(|w| w == b"private-token-fixture")
                    );
                    assert!(
                        !bytes
                            .windows(b"replacement-fixture".len())
                            .any(|w| w == b"replacement-fixture")
                    );
                }
            }
        }
        walk(self.root.path());
    }
}
fn connection(c: &RemoteCredentialSummary) -> RemoteConnection {
    RemoteConnection {
        name: "origin".into(),
        url: format!("https://{}/owner/notes.git", c.host),
        branch: "main".into(),
        bookmark: "main".into(),
        authentication: if matches!(c.method, RemoteCredentialMethod::Token) {
            RemoteAuthentication::Token
        } else {
            RemoteAuthentication::Forge
        },
        credential: c.id.clone(),
    }
}
#[tokio::test]
async fn encrypted_provider_lifecycle_survives_restart_and_preserves_owner_isolation() {
    let f = Fixture::new();
    let store = f.store();
    let provider = f.provider(&store, f.owner);
    for (kind, host) in [
        (RemoteCredentialProvider::Https, "git.example"),
        (RemoteCredentialProvider::Github, "github.com"),
        (RemoteCredentialProvider::Forgejo, "forge.example"),
    ] {
        provider
            .control(Action::Save(f.input(kind, host)))
            .await
            .unwrap();
    }
    let credentials = provider.inventory().unwrap().credentials;
    assert_eq!(credentials.len(), 3);
    f.no_plaintext();
    assert!(
        !serde_json::to_string(&provider.inventory().unwrap())
            .unwrap()
            .contains("private-token-fixture")
    );
    let foreign = f.provider(&store, Uuid::new_v4());
    assert!(foreign.inventory().unwrap().credentials.is_empty());
    for saved in &credentials {
        let c = connection(saved);
        provider.resolve_async(&c).await.unwrap();
        assert!(foreign.resolve_async(&c).await.is_err());
        assert!(
            foreign
                .control(Action::Forget(saved.id.clone()))
                .await
                .is_err()
        );
        let mut replacement = f.input(
            if matches!(saved.method, RemoteCredentialMethod::Token) {
                RemoteCredentialProvider::Https
            } else if saved.host == "forge.example" {
                RemoteCredentialProvider::Forgejo
            } else {
                RemoteCredentialProvider::Github
            },
            &saved.host,
        );
        replacement.replace = Some(saved.id.clone());
        replacement.token = "replacement-fixture".into();
        provider.control(Action::Save(replacement)).await.unwrap();
    }
    drop(foreign);
    drop(provider);
    drop(store);
    let store = f.store();
    let provider = f.provider(&store, f.owner);
    let restarted = provider.inventory().unwrap();
    assert_eq!(restarted.credentials.len(), 3);
    for saved in &credentials {
        provider.resolve_async(&connection(saved)).await.unwrap();
        provider
            .control(Action::Forget(saved.id.clone()))
            .await
            .unwrap();
    }
    assert!(provider.inventory().unwrap().credentials.is_empty());
    f.no_plaintext();
}
#[tokio::test]
async fn missing_wrong_and_lost_keys_show_locked_without_an_os_or_plaintext_fallback() {
    let f = Fixture::new();
    let store = f.store();
    let provider = f.provider(&store, f.owner);
    provider
        .control(Action::Save(
            f.input(RemoteCredentialProvider::Github, "github.com"),
        ))
        .await
        .unwrap();
    let saved = provider.inventory().unwrap().credentials.remove(0);
    drop(provider);
    drop(store);
    let wrong = f.root.path().join("wrong");
    fs::write(&wrong, [8u8; 32]).unwrap();
    for key in [None, Some(wrong.as_path())] {
        let store = Arc::new(CredentialStore::open(
            &f.root.path().join("private"),
            f.installation,
            key,
        ));
        let provider = f.provider(&store, f.owner);
        let settings = provider.inventory().unwrap();
        assert!(matches!(settings.storage, RemoteStorageState::Locked));
        assert!(settings.ssh_available);
        assert!(!settings.device_sign_in_available);
        assert!(provider.resolve_async(&connection(&saved)).await.is_err());
        assert!(
            provider
                .control(Action::Save(
                    f.input(RemoteCredentialProvider::Https, "git.example")
                ))
                .await
                .is_err()
        );
    }
    let store = f.store();
    let provider = f.provider(&store, f.owner);
    provider.resolve_async(&connection(&saved)).await.unwrap();
    f.no_plaintext();
}
