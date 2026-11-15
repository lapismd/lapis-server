use crate::credentials::{HASH, NOW, PASSWORD};
use axum::{
    body::Body,
    extract::Request,
    middleware::{self, Next},
    response::Response,
};
use lapis_app_core::{
    sync::account::{
        AccountEndpoint, AccountTransportPolicy,
        protected::{HostAccountCustody, HostSecretBytes, ProtectedAccountState},
        session::AccountSession,
    },
    vault::VaultRoot,
};
use lapis_server::{
    config::Config,
    control::Control,
    host::Host,
    http::{Authentication, router},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, AtomicUsize, Ordering},
    },
};
use uuid::Uuid;

#[derive(Default)]
pub struct Faults {
    pub lose: AtomicU8,
    pub initiations: AtomicUsize,
    pub redemptions: AtomicUsize,
    pub signouts: AtomicUsize,
    pub cancellations: AtomicUsize,
}
impl Faults {
    async fn route(&self, request: Request, next: Next) -> Response {
        let path = request.uri().path();
        let kind = if path == "/api/v3/sync/account" {
            self.initiations.fetch_add(1, Ordering::SeqCst);
            3
        } else if path.ends_with("/redeem") {
            self.redemptions.fetch_add(1, Ordering::SeqCst);
            1
        } else if path.ends_with("/sign-out") {
            self.signouts.fetch_add(1, Ordering::SeqCst);
            2
        } else if path.ends_with("/cancel") {
            self.cancellations.fetch_add(1, Ordering::SeqCst);
            4
        } else {
            0
        };
        let mut response = next.run(request).await;
        if kind != 0
            && response.status().is_success()
            && self
                .lose
                .compare_exchange(kind, 0, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        {
            // The real handler committed. Remove its successful response body.
            *response.body_mut() = Body::from("simulated-lost-acknowledgement");
        }
        response
    }
}
/// Explicit test-only fake, outside profile and vault; never an OS custody pass.
pub struct FileCustody {
    path: PathBuf,
    gate: Mutex<()>,
    pub fail_write: AtomicUsize,
    writes: AtomicUsize,
}
impl FileCustody {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            gate: Mutex::new(()),
            fail_write: AtomicUsize::new(0),
            writes: AtomicUsize::new(0),
        }
    }
    fn values(&self) -> BTreeMap<String, Vec<u8>> {
        if !self.path.exists() {
            BTreeMap::new()
        } else {
            serde_json::from_slice(&std::fs::read(&self.path).unwrap()).unwrap()
        }
    }
}
impl HostAccountCustody for FileCustody {
    fn supported(&self) -> bool {
        true
    }
    fn read(&self, key: &str) -> io::Result<Option<HostSecretBytes>> {
        let _guard = self.gate.lock().unwrap();
        self.values()
            .remove(key)
            .map(HostSecretBytes::new)
            .transpose()
    }
    fn write(&self, key: &str, value: &HostSecretBytes) -> io::Result<()> {
        let _guard = self.gate.lock().unwrap();
        let count = self.writes.fetch_add(1, Ordering::SeqCst) + 1;
        let mut values = self.values();
        values.insert(key.into(), value.expose_secret().to_vec());
        std::fs::write(&self.path, serde_json::to_vec(&values).unwrap())?;
        if self.fail_write.load(Ordering::SeqCst) == count {
            return Err(io::Error::other("synthetic-custody-acknowledgement-loss"));
        }
        Ok(())
    }
}
pub struct Device {
    pub root: tempfile::TempDir,
    pub profile: Uuid,
    pub vault: Uuid,
    pub custody: Arc<FileCustody>,
}
impl Device {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        for name in ["profile", "vault"] {
            std::fs::create_dir(root.path().join(name)).unwrap();
        }
        let custody = Arc::new(FileCustody::new(root.path().join("fake-custody.json")));
        Self {
            root,
            profile: Uuid::new_v4(),
            vault: Uuid::new_v4(),
            custody,
        }
    }
    pub fn session(&self, origin: &str) -> AccountSession {
        open(
            self.root.path(),
            self.profile,
            self.vault,
            origin,
            self.custody.clone(),
        )
    }
    pub fn descriptor(&self, origin: &str, client: Uuid, managed: Uuid) -> PathBuf {
        let path = self.root.path().join("cold-public-scope.json");
        std::fs::write(&path, serde_json::to_vec(&json!({"root":self.root.path(),"profile":self.profile,"vault":self.vault,"origin":origin,"client":client,"managed":managed})).unwrap()).unwrap();
        path
    }
}
pub fn open(
    root: &Path,
    profile: Uuid,
    vault: Uuid,
    origin: &str,
    custody: Arc<FileCustody>,
) -> AccountSession {
    let endpoint =
        AccountEndpoint::open(origin, AccountTransportPolicy::LoopbackDevelopment).unwrap();
    let state = ProtectedAccountState::open(
        &VaultRoot::open(&root.join("profile")).unwrap(),
        &VaultRoot::open(&root.join("vault")).unwrap(),
        profile,
        vault,
        &endpoint,
        custody,
    )
    .unwrap();
    AccountSession::new(state, endpoint)
}
pub struct Server {
    _root: tempfile::TempDir,
    pub host: Arc<Host>,
    control: Arc<Control>,
    pub origin: String,
    client: reqwest::Client,
    cookie: String,
    csrf: String,
    pub managed: Uuid,
    pub faults: Arc<Faults>,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    pub async fn start() -> Self {
        let root = tempfile::tempdir().unwrap();
        let profile = root.path().join("profile");
        std::fs::create_dir(&profile).unwrap();
        let host = Arc::new(Host::with_profile(Config { vaults: vec![] }, &profile).unwrap());
        let control = Arc::new(Control::open(&profile).await.unwrap());
        control.initialize("Owner", HASH, NOW).await.unwrap();
        host.enable_management(control.clone(), &profile)
            .await
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://localhost:{}", listener.local_addr().unwrap().port());
        let faults = Arc::new(Faults::default());
        let fault = faults.clone();
        let app = router(
            host.clone(),
            Authentication::browser(control.clone(), None, true).unwrap(),
            &origin,
        )
        .unwrap()
        .layer(middleware::from_fn(move |request, next| {
            let fault = fault.clone();
            async move { fault.route(request, next).await }
        }));
        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
            .unwrap();
        });
        let client = reqwest::Client::new();
        let login = client
            .post(format!("{origin}/api/v3/auth/login"))
            .header("origin", &origin)
            .header("x-lapis-protocol", "3")
            .json(&json!({"username":"owner","password":PASSWORD}))
            .send()
            .await
            .unwrap();
        assert_eq!(login.status(), 200);
        let cookie = login.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        let info: Value = login.json().await.unwrap();
        let csrf = info["csrf_token"].as_str().unwrap().to_owned();
        let managed = client
            .post(format!("{origin}/api/v3/vaults"))
            .header("origin", &origin)
            .header("x-lapis-protocol", "3")
            .header("cookie", &cookie)
            .header("x-lapis-csrf", &csrf)
            .json(&json!({"name":"Managed source acceptance"}))
            .send()
            .await
            .unwrap();
        assert_eq!(managed.status(), 201);
        let managed: Value = managed.json().await.unwrap();
        Self {
            _root: root,
            host,
            control,
            origin,
            client,
            cookie,
            csrf,
            managed: serde_json::from_value(managed["vault_id"].clone()).unwrap(),
            faults,
            task,
        }
    }
    pub async fn approve(&self, id: Uuid) {
        let response = self
            .client
            .post(format!("{}/api/v3/sync/account/{id}/decision", self.origin))
            .header("origin", &self.origin)
            .header("x-lapis-protocol", "3")
            .header("cookie", &self.cookie)
            .header("x-lapis-csrf", &self.csrf)
            .json(&json!({"approve":true}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }
    pub async fn finish(&mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
        let host = self.host.clone();
        tokio::task::spawn_blocking(move || host.shutdown())
            .await
            .unwrap()
            .unwrap();
        self.control.close().await.unwrap();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
