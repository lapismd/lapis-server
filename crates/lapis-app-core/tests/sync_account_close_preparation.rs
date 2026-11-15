use lapis_app_core::{
    sync::account::{
        AccountEndpoint, AccountTransportPolicy,
        owner::{AccountCommand, AccountObservation, AccountOwner, AccountReply},
        protected::{HostAccountCustody, HostSecretBytes, ProtectedAccountState},
        session::AccountSession,
    },
    vault::VaultRoot,
};
use std::{
    collections::HashMap,
    io,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
use uuid::Uuid;

#[derive(Default)]
struct Custody {
    values: Mutex<HashMap<String, Vec<u8>>>,
    fail: AtomicBool,
    pause: AtomicBool,
    entered: Mutex<Option<mpsc::Sender<()>>>,
    resume: (Mutex<bool>, Condvar),
}
impl HostAccountCustody for Custody {
    fn supported(&self) -> bool {
        true
    }
    fn read(&self, key: &str) -> io::Result<Option<HostSecretBytes>> {
        if self.pause.swap(false, Ordering::SeqCst) {
            if let Some(sender) = self.entered.lock().unwrap().as_ref() {
                let _ = sender.send(());
            }
            let mut resumed = self.resume.0.lock().unwrap();
            while !*resumed {
                resumed = self.resume.1.wait(resumed).unwrap();
            }
        }
        self.values
            .lock()
            .unwrap()
            .get(key)
            .map(|v| HostSecretBytes::new(v.clone()))
            .transpose()
    }
    fn write(&self, key: &str, value: &HostSecretBytes) -> io::Result<()> {
        if self.fail.swap(false, Ordering::SeqCst) {
            return Err(io::Error::other("synthetic custody failure"));
        }
        self.values
            .lock()
            .unwrap()
            .insert(key.into(), value.expose_secret().to_vec());
        Ok(())
    }
}

struct Resume(Arc<Custody>);
impl Drop for Resume {
    fn drop(&mut self) {
        *self.0.resume.0.lock().unwrap() = true;
        self.0.resume.1.notify_all();
    }
}

#[test]
fn preparation_drains_detached_work_with_one_blocking_worker_and_abandon_keeps_the_owner() {
    let fixture = Fixture::new();
    let owner = AccountOwner::new(fixture.session());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let original = client(&owner).await;
        let (entered, received) = mpsc::channel();
        *fixture.custody.entered.lock().unwrap() = Some(entered);
        fixture.custody.pause.store(true, Ordering::SeqCst);
        let resume = Resume(fixture.custody.clone());
        let work = owner.submit(AccountCommand::Snapshot).unwrap();
        received
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let preparation = owner.prepare_close().unwrap();
        drop(work);
        let busy = owner.submit(AccountCommand::Snapshot).err().unwrap().kind();
        drop(resume);
        assert_eq!(busy, io::ErrorKind::WouldBlock);
        let guard = tokio::time::timeout(std::time::Duration::from_secs(5), preparation.wait())
            .await
            .unwrap()
            .unwrap();
        drop(guard);
        assert_eq!(client(&owner).await, original);
    });
}

#[test]
fn prepared_scope_keeps_its_process_lock_until_commit_and_restores_the_same_client() {
    const PROFILE: &str = "LAPIS_SYNC_PREPARE_TEST_PROFILE";
    const VAULT: &str = "LAPIS_SYNC_PREPARE_TEST_VAULT";
    const PROFILE_ID: &str = "LAPIS_SYNC_PREPARE_TEST_PROFILE_ID";
    const VAULT_ID: &str = "LAPIS_SYNC_PREPARE_TEST_VAULT_ID";
    const CLIENT: &str = "LAPIS_SYNC_PREPARE_TEST_CLIENT";
    const BLOCKED: &str = "LAPIS_SYNC_PREPARE_TEST_BLOCKED";
    if let Some(profile) = std::env::var_os(PROFILE) {
        let vault = std::env::var_os(VAULT).unwrap();
        let endpoint =
            AccountEndpoint::open("https://sync.example", AccountTransportPolicy::Https).unwrap();
        let opened = ProtectedAccountState::open(
            &VaultRoot::open(std::path::Path::new(&profile)).unwrap(),
            &VaultRoot::open(std::path::Path::new(&vault)).unwrap(),
            Uuid::parse_str(&std::env::var(PROFILE_ID).unwrap()).unwrap(),
            Uuid::parse_str(&std::env::var(VAULT_ID).unwrap()).unwrap(),
            &endpoint,
            Arc::new(Custody::default()),
        );
        if std::env::var(BLOCKED).unwrap() == "true" {
            assert!(opened.is_err());
        } else {
            assert_eq!(
                opened.unwrap().client_id().to_string(),
                std::env::var(CLIENT).unwrap()
            );
        }
        return;
    }
    let fixture = Fixture::new();
    let owner = AccountOwner::new(fixture.session());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let original = client(&owner).await;
        let child = |blocked: bool| {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "prepared_scope_keeps_its_process_lock_until_commit_and_restores_the_same_client", "--nocapture"])
                .env(PROFILE, fixture.profile.path()).env(VAULT, fixture.vault.path())
                .env(PROFILE_ID, fixture.profile_id.to_string()).env(VAULT_ID, fixture.vault_id.to_string())
                .env(CLIENT, original.to_string()).env(BLOCKED, blocked.to_string()).output().unwrap();
            assert!(output.status.success(), "cold child: {}", String::from_utf8_lossy(&output.stderr));
        };
        let guard = owner.prepare_close().unwrap().wait().await.unwrap();
        child(true);
        drop(guard);
        child(true);
        let guard = owner.prepare_close().unwrap().wait().await.unwrap();
        tokio::task::spawn_blocking(move || guard.commit()).await.unwrap().unwrap();
        child(false);
    });
}
struct Fixture {
    _root: tempfile::TempDir,
    profile: VaultRoot,
    vault: VaultRoot,
    profile_id: Uuid,
    vault_id: Uuid,
    custody: Arc<Custody>,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("profile")).unwrap();
        std::fs::create_dir(root.path().join("vault")).unwrap();
        Self {
            profile: VaultRoot::open(&root.path().join("profile")).unwrap(),
            vault: VaultRoot::open(&root.path().join("vault")).unwrap(),
            _root: root,
            profile_id: Uuid::new_v4(),
            vault_id: Uuid::new_v4(),
            custody: Arc::new(Custody::default()),
        }
    }
    fn session(&self) -> AccountSession {
        let endpoint =
            AccountEndpoint::open("https://sync.example", AccountTransportPolicy::Https).unwrap();
        let state = ProtectedAccountState::open(
            &self.profile,
            &self.vault,
            self.profile_id,
            self.vault_id,
            &endpoint,
            self.custody.clone(),
        )
        .unwrap();
        AccountSession::new(state, endpoint)
    }
}
async fn client(owner: &AccountOwner) -> Uuid {
    match owner
        .submit(AccountCommand::Snapshot)
        .unwrap()
        .wait()
        .await
        .unwrap()
    {
        AccountReply::Snapshot(value) => value.client_id,
        _ => panic!("expected snapshot"),
    }
}
#[test]
fn refused_host_leave_drops_preparation_and_retains_the_original_account_owner() {
    let fixture = Fixture::new();
    let owner = AccountOwner::new(fixture.session());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let original = client(&owner).await;
        let guard = owner.prepare_close().unwrap().wait().await.unwrap();
        assert_eq!(
            owner.submit(AccountCommand::Snapshot).err().unwrap().kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(
            owner.submit(AccountCommand::Close).err().unwrap().kind(),
            io::ErrorKind::WouldBlock
        );
        // Another native owner rejects leave; dropping admission must not close custody.
        drop(guard);
        assert_eq!(client(&owner).await, original);
        let guard = owner.prepare_close().unwrap().wait().await.unwrap();
        tokio::task::spawn_blocking(move || guard.commit())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(*owner.subscribe().borrow(), AccountObservation::Closed);
        assert_eq!(
            owner
                .submit(AccountCommand::Snapshot)
                .unwrap()
                .wait()
                .await
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::NotConnected
        );
    });
    assert!(fixture.custody.values.lock().unwrap().is_empty());
}

#[test]
fn uncertain_custody_refuses_preparation_then_explicit_recovery_can_commit_close() {
    let fixture = Fixture::new();
    let owner = AccountOwner::new(fixture.session());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let original = client(&owner).await;
        fixture.custody.fail.store(true, Ordering::SeqCst);
        assert!(
            owner
                .submit(AccountCommand::Begin {
                    name: "Desktop".into(),
                    now: 1
                })
                .unwrap()
                .wait()
                .await
                .is_err()
        );
        assert!(owner.prepare_close().unwrap().wait().await.is_err());
        assert_eq!(
            *owner.subscribe().borrow(),
            AccountObservation::RecoveryRequired {
                custody_pending: true
            }
        );
        owner
            .submit(AccountCommand::RetryCustody)
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert_eq!(client(&owner).await, original);
        let guard = owner.prepare_close().unwrap().wait().await.unwrap();
        tokio::task::spawn_blocking(move || guard.commit())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(*owner.subscribe().borrow(), AccountObservation::Closed);
    });
}
