use jj_forge::{
    Result as ForgeResult,
    accounts::{Manager, http::Transport, store::SecretStore},
};
use lapis_app_core::services::{
    NativeRemoteCredentials, RemoteCredentialAction as Action, RemoteCredentials, RemoteTokenStore,
};
use lapis_app_protocol::repository_remotes::*;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

#[derive(Default)]
struct Secrets(Mutex<HashMap<String, String>>);
impl SecretStore for Secrets {
    fn get(&self, id: &str) -> ForgeResult<Option<String>> {
        Ok(self.0.lock().unwrap().get(id).cloned())
    }
    fn set(&self, id: &str, value: &str) -> ForgeResult<()> {
        self.0.lock().unwrap().insert(id.into(), value.into());
        Ok(())
    }
    fn remove(&self, id: &str) -> ForgeResult<()> {
        self.0.lock().unwrap().remove(id);
        Ok(())
    }
}
#[derive(Default)]
struct Network(Mutex<Vec<String>>, AtomicBool);
#[async_trait::async_trait(?Send)]
impl Transport for Network {
    async fn request(
        &self,
        url: &str,
        token: Option<&str>,
        _body: Option<Value>,
    ) -> ForgeResult<Value> {
        self.0.lock().unwrap().push(url.into());
        if url.ends_with("/login/device/code") {
            return Ok(
                json!({"device_code":"secret-device-fixture", "user_code":"ABCD-EFGH", "verification_uri":"https://github.com/login/device", "expires_in":if self.1.load(Ordering::SeqCst) {1} else {900}, "interval":5}),
            );
        }
        if url.ends_with("/login/oauth/access_token") {
            return Ok(
                json!({"access_token":"oauth-token-fixture", "refresh_token":"refresh-fixture", "expires_in":900}),
            );
        }
        Ok(json!({"id":if token == Some("wrong-user-fixture") {99} else {42}, "login":"fixture"}))
    }
}
#[derive(Default)]
struct Tokens {
    locked: AtomicBool,
    values: Mutex<HashMap<String, (RemoteCredentialSummary, String)>>,
    resolves: AtomicUsize,
}
impl RemoteTokenStore for Tokens {
    fn state(&self) -> RemoteStorageState {
        if self.locked.load(Ordering::SeqCst) {
            RemoteStorageState::Locked
        } else {
            RemoteStorageState::Available
        }
    }
    fn inventory(&self) -> io::Result<Vec<RemoteCredentialSummary>> {
        Ok(self
            .values
            .lock()
            .unwrap()
            .values()
            .map(|v| v.0.clone())
            .collect())
    }
    fn save(&self, input: &RemoteCredentialInput) -> io::Result<()> {
        let id = input
            .replace
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        self.values.lock().unwrap().insert(
            id.clone(),
            (
                RemoteCredentialSummary {
                    id,
                    label: input.label.clone(),
                    host: input.host.clone(),
                    username: input.username.clone(),
                    method: RemoteCredentialMethod::Token,
                    available: true,
                },
                input.token.clone(),
            ),
        );
        Ok(())
    }
    fn forget(&self, id: &str) -> io::Result<()> {
        self.values.lock().unwrap().remove(id);
        Ok(())
    }
    fn resolve(&self, c: &RemoteConnection) -> io::Result<jj_vault::GitCredentials> {
        self.resolves.fetch_add(1, Ordering::SeqCst);
        let values = self.values.lock().unwrap();
        let (saved, token) = values.get(&c.credential).unwrap();
        jj_vault::GitCredentials::basic(&saved.username, token)
            .map_err(|_| io::Error::other("fixture"))
    }
}
struct Fixture {
    _root: tempfile::TempDir,
    manager: Arc<Manager>,
    secrets: Arc<Secrets>,
    network: Arc<Network>,
    tokens: Arc<Tokens>,
    provider: NativeRemoteCredentials,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let secrets = Arc::new(Secrets::default());
        let network = Arc::new(Network::default());
        let tokens = Arc::new(Tokens::default());
        let manager = Arc::new(Manager::new(
            root.path().join("accounts"),
            secrets.clone(),
            network.clone(),
            Some("fixtureclient".into()),
        ));
        let provider =
            NativeRemoteCredentials::new(RemoteHost::Server, tokens.clone(), manager.clone(), true);
        Self {
            _root: root,
            manager,
            secrets,
            network,
            tokens,
            provider,
        }
    }
    fn restart(&self) -> NativeRemoteCredentials {
        NativeRemoteCredentials::new(
            RemoteHost::Desktop,
            self.tokens.clone(),
            self.manager.clone(),
            false,
        )
    }
}
fn input(provider: RemoteCredentialProvider, host: &str) -> RemoteCredentialInput {
    RemoteCredentialInput {
        label: "Fixture connection".into(),
        host: host.into(),
        username: "fixture".into(),
        token: "private-token-fixture".into(),
        provider,
        replace: None,
    }
}
fn connection(saved: &RemoteCredentialSummary) -> RemoteConnection {
    RemoteConnection {
        name: "origin".into(),
        url: format!("https://{}/owner/repo.git", saved.host),
        branch: "main".into(),
        bookmark: "publish".into(),
        authentication: if matches!(saved.method, RemoteCredentialMethod::Token) {
            RemoteAuthentication::Token
        } else {
            RemoteAuthentication::Forge
        },
        credential: saved.id.clone(),
    }
}

#[tokio::test]
async fn raw_token_lifecycle_and_cross_scope_refusal_precede_custody_resolution() {
    let f = Fixture::new();
    f.provider
        .control(Action::Save(input(
            RemoteCredentialProvider::Https,
            "git.example",
        )))
        .await
        .unwrap();
    assert!(f.network.0.lock().unwrap().is_empty());
    let settings = f.provider.inventory().unwrap();
    let saved = &settings.credentials[0];
    assert!(
        !serde_json::to_string(&settings)
            .unwrap()
            .contains("private-token-fixture")
    );
    let c = connection(saved);
    f.provider.resolve_async(&c).await.unwrap();
    assert_eq!(f.tokens.resolves.load(Ordering::SeqCst), 1);
    for url in [
        "https://elsewhere.example/owner/repo.git",
        "https://git.example:8443/repo.git",
        "https://user:secret@git.example/repo.git",
        "http://git.example/repo.git",
        "https://git.example/repo.git?token=secret",
    ] {
        let mut denied = c.clone();
        denied.url = url.into();
        assert!(f.provider.resolve_async(&denied).await.is_err());
    }
    let mut wrong_method = c.clone();
    wrong_method.authentication = RemoteAuthentication::Forge;
    assert!(f.provider.resolve_async(&wrong_method).await.is_err());
    assert_eq!(f.tokens.resolves.load(Ordering::SeqCst), 1);
    for (host, username) in [("elsewhere.example", "fixture"), ("git.example", "other")] {
        let mut replacement = input(RemoteCredentialProvider::Https, host);
        replacement.username = username.into();
        replacement.replace = Some(saved.id.clone());
        assert!(f.provider.control(Action::Save(replacement)).await.is_err());
    }
    let mut replacement = input(RemoteCredentialProvider::Https, "git.example");
    replacement.replace = Some(saved.id.clone());
    replacement.token = "replacement-fixture".into();
    f.provider.control(Action::Save(replacement)).await.unwrap();
    assert!(f.tokens.values.lock().unwrap().get(&saved.id).unwrap().1 == "replacement-fixture");
    let restarted = f.restart();
    assert_eq!(restarted.inventory().unwrap().credentials[0].id, saved.id);
    restarted
        .control(Action::Forget(saved.id.clone()))
        .await
        .unwrap();
    assert!(restarted.inventory().unwrap().credentials.is_empty());
}
#[tokio::test]
async fn forge_lifecycle_keeps_verified_identity_and_scope_in_both_host_compositions() {
    for (provider, host) in [
        (RemoteCredentialProvider::Github, "github.com"),
        (RemoteCredentialProvider::Github, "enterprise.example:8443"),
        (RemoteCredentialProvider::Forgejo, "forge.example"),
    ] {
        let f = Fixture::new();
        f.provider
            .control(Action::Save(input(provider, host)))
            .await
            .unwrap();
        let saved = f.provider.inventory().unwrap().credentials.remove(0);
        assert_eq!(saved.host, host);
        assert_eq!(saved.username, "fixture");
        assert!(f.tokens.values.lock().unwrap().is_empty());
        f.provider.resolve_async(&connection(&saved)).await.unwrap();
        let secret = f.secrets.get(&saved.id).unwrap();
        let mut replacement = input(provider, host);
        replacement.replace = Some(saved.id.clone());
        replacement.token = "wrong-user-fixture".into();
        assert!(f.provider.control(Action::Save(replacement)).await.is_err());
        assert!(f.secrets.get(&saved.id).unwrap() == secret);
        let calls = f.network.0.lock().unwrap().len();
        let mut wrong_provider = input(
            if matches!(provider, RemoteCredentialProvider::Forgejo) {
                RemoteCredentialProvider::Github
            } else {
                RemoteCredentialProvider::Forgejo
            },
            host,
        );
        wrong_provider.replace = Some(saved.id.clone());
        assert!(
            f.provider
                .control(Action::Save(wrong_provider))
                .await
                .is_err()
        );
        assert_eq!(f.network.0.lock().unwrap().len(), calls);
        let mut replacement = input(provider, host);
        replacement.replace = Some(saved.id.clone());
        replacement.token = "replacement-fixture".into();
        f.provider.control(Action::Save(replacement)).await.unwrap();
        assert_eq!(f.provider.inventory().unwrap().credentials[0].id, saved.id);
        assert!(
            f.manager
                .credential(&saved.id, jj_forge::accounts::now().unwrap())
                .await
                .unwrap()
                .access_token
                == "replacement-fixture"
        );
        let restarted = f.restart();
        restarted.resolve_async(&connection(&saved)).await.unwrap();
        restarted.control(Action::Forget(saved.id)).await.unwrap();
        assert!(restarted.inventory().unwrap().credentials.is_empty());
    }
}
#[tokio::test]
async fn locked_custody_preserves_host_ssh_but_refuses_all_saved_secret_operations() {
    let f = Fixture::new();
    f.provider
        .control(Action::Save(input(
            RemoteCredentialProvider::Github,
            "github.com",
        )))
        .await
        .unwrap();
    let saved = f.provider.inventory().unwrap().credentials.remove(0);
    let calls = f.network.0.lock().unwrap().len();
    f.tokens.locked.store(true, Ordering::SeqCst);
    let locked = f.provider.inventory().unwrap();
    assert!(matches!(locked.storage, RemoteStorageState::Locked));
    assert!(locked.ssh_available);
    assert!(!locked.device_sign_in_available);
    assert!(locked.credentials.is_empty());
    for action in [
        Action::Save(input(RemoteCredentialProvider::Https, "github.com")),
        Action::Forget(saved.id.clone()),
        Action::BeginSignIn,
        Action::CheckSignIn,
    ] {
        assert!(f.provider.control(action).await.is_err());
    }
    assert!(f.provider.resolve_async(&connection(&saved)).await.is_err());
    assert_eq!(f.network.0.lock().unwrap().len(), calls);
    assert!(f.secrets.get(&saved.id).unwrap().is_some());
}
#[tokio::test]
async fn device_flow_uses_sdk_intervals_and_never_exposes_private_codes_or_sessions() {
    let f = Fixture::new();
    f.provider.control(Action::BeginSignIn).await.unwrap();
    let pending = f.provider.inventory().unwrap();
    let auth = pending.authorization.as_ref().unwrap();
    assert_eq!(auth.code, "ABCD-EFGH");
    assert_eq!(auth.url, "https://github.com/login/device");
    let public = serde_json::to_string(&pending).unwrap();
    assert!(!public.contains("secret-device-fixture"));
    assert!(!public.contains("session"));
    assert!(f.provider.control(Action::BeginSignIn).await.is_err());
    f.provider.control(Action::CheckSignIn).await.unwrap();
    assert_eq!(f.network.0.lock().unwrap().len(), 1);
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    f.provider.control(Action::CheckSignIn).await.unwrap();
    let done = f.provider.inventory().unwrap();
    assert!(done.authorization.is_none());
    assert_eq!(done.credentials.len(), 1);
    assert_eq!(f.network.0.lock().unwrap().len(), 3);
    assert!(
        !serde_json::to_string(&done)
            .unwrap()
            .contains("oauth-token-fixture")
    );
    f.provider
        .resolve_async(&connection(&done.credentials[0]))
        .await
        .unwrap();
    f.provider.control(Action::BeginSignIn).await.unwrap();
    f.provider.control(Action::CancelSignIn).await.unwrap();
    assert!(f.provider.inventory().unwrap().authorization.is_none());
    assert!(f.provider.control(Action::CheckSignIn).await.is_err());
}
#[tokio::test]
async fn expired_sign_in_remains_publicly_expired_without_polling_for_a_token() {
    let f = Fixture::new();
    f.network.1.store(true, Ordering::SeqCst);
    f.provider.control(Action::BeginSignIn).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    assert!(matches!(
        f.provider.inventory().unwrap().authorization.unwrap().state,
        RemoteAuthorizationState::Expired
    ));
    f.provider.control(Action::CheckSignIn).await.unwrap();
    assert!(matches!(
        f.provider.inventory().unwrap().authorization.unwrap().state,
        RemoteAuthorizationState::Expired
    ));
    assert_eq!(f.network.0.lock().unwrap().len(), 1);
    f.provider.control(Action::CancelSignIn).await.unwrap();
}
