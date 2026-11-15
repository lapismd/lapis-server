use lapis_app_core::{
    sync::account::{
        AccountEndpoint, AccountTransportPolicy,
        owner::{AccountCommand, AccountObservation, AccountOwner, AccountReply},
        protected::{HostAccountCustody, HostSecretBytes, ProtectedAccountState},
        session::{AccountPhase, AccountSession},
    },
    vault::VaultRoot,
};
use std::{
    collections::HashMap,
    io,
    sync::{
        Arc, Barrier, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

#[derive(Default)]
struct Custody {
    values: Mutex<HashMap<String, Vec<u8>>>,
    fail: AtomicBool,
    locked: AtomicBool,
    pause: AtomicBool,
    entered: Option<Arc<Barrier>>,
    released: (Mutex<bool>, Condvar),
}
impl HostAccountCustody for Custody {
    fn supported(&self) -> bool {
        true
    }
    fn read(&self, key: &str) -> io::Result<Option<HostSecretBytes>> {
        if self.locked.load(Ordering::SeqCst) {
            return Err(io::Error::other("private-custody-detail"));
        }
        self.values
            .lock()
            .unwrap()
            .get(key)
            .map(|v| HostSecretBytes::new(v.clone()))
            .transpose()
    }
    fn write(&self, key: &str, value: &HostSecretBytes) -> io::Result<()> {
        if self.pause.swap(false, Ordering::SeqCst) {
            self.entered.as_ref().unwrap().wait();
            let (lock, wake) = &self.released;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = wake.wait(released).unwrap();
            }
        }
        if self.fail.swap(false, Ordering::SeqCst) {
            return Err(io::Error::other("synthetic custody refusal"));
        }
        self.values
            .lock()
            .unwrap()
            .insert(key.into(), value.expose_secret().to_vec());
        Ok(())
    }
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
    fn new(custody: Arc<Custody>) -> Self {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("profile")).unwrap();
        std::fs::create_dir(root.path().join("vault")).unwrap();
        Self {
            profile: VaultRoot::open(&root.path().join("profile")).unwrap(),
            vault: VaultRoot::open(&root.path().join("vault")).unwrap(),
            _root: root,
            profile_id: Uuid::new_v4(),
            vault_id: Uuid::new_v4(),
            custody,
        }
    }
    fn session(&self) -> io::Result<AccountSession> {
        let endpoint =
            AccountEndpoint::open("https://sync.example", AccountTransportPolicy::Https)?;
        let store = ProtectedAccountState::open(
            &self.profile,
            &self.vault,
            self.profile_id,
            self.vault_id,
            &endpoint,
            self.custody.clone(),
        )?;
        Ok(AccountSession::new(store, endpoint))
    }
}

#[test]
fn detached_work_stays_single_flight_and_failed_close_retains_custody_owner() {
    let entered = Arc::new(Barrier::new(2));
    let custody = Arc::new(Custody {
        entered: Some(entered.clone()),
        ..Default::default()
    });
    let f = Fixture::new(custody.clone());
    let session = f.session().unwrap();
    let client = session.snapshot().unwrap().client_id;
    let owner = AccountOwner::new(session);
    let mut observations = owner.subscribe();
    assert_eq!(*observations.borrow(), AccountObservation::Uninspected);
    custody.fail.store(true, Ordering::SeqCst);
    custody.pause.store(true, Ordering::SeqCst);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let work = owner
            .submit(AccountCommand::Begin {
                name: "Desktop".into(),
                now: 1,
            })
            .unwrap();
        entered.wait();
        drop(work);
        assert_eq!(
            owner.submit(AccountCommand::Snapshot).err().unwrap().kind(),
            io::ErrorKind::WouldBlock
        );
        *custody.released.0.lock().unwrap() = true;
        custody.released.1.notify_all();
        tokio::time::timeout(Duration::from_secs(5), observations.changed())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            *observations.borrow_and_update(),
            AccountObservation::RecoveryRequired {
                custody_pending: true
            }
        );
        // A completion notification can precede permit release by a few
        // instructions. Bound explicit read admission and await its completion.
        let read = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match owner.submit(AccountCommand::Snapshot) {
                    Ok(work) => break work,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        tokio::task::yield_now().await
                    }
                    Err(e) => panic!("read admission: {e}"),
                }
            }
        })
        .await
        .unwrap();
        assert!(read.wait().await.is_err());
        assert!(
            owner
                .submit(AccountCommand::Close)
                .unwrap()
                .wait()
                .await
                .is_err()
        );
    });
    assert!(
        f.session().is_err(),
        "failed close must retain the physical owner lock"
    );
    runtime.block_on(async {
        let AccountReply::Snapshot(snapshot) = owner
            .submit(AccountCommand::RetryCustody)
            .unwrap()
            .wait()
            .await
            .unwrap()
        else {
            panic!("snapshot")
        };
        assert_eq!(snapshot.phase, AccountPhase::Prepared);
        assert_eq!(snapshot.client_id, client);
        owner
            .submit(AccountCommand::AbandonUnissued { now: 1 })
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert!(matches!(
            owner
                .submit(AccountCommand::Close)
                .unwrap()
                .wait()
                .await
                .unwrap(),
            AccountReply::Closed
        ));
        assert_eq!(*observations.borrow(), AccountObservation::Closed);
        assert_eq!(
            owner
                .submit(AccountCommand::Begin {
                    name: "Desktop".into(),
                    now: 2
                })
                .unwrap()
                .wait()
                .await
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::NotConnected
        );
    });
    let mut restored = f.session().unwrap();
    assert_eq!(restored.snapshot().unwrap().client_id, client);
    assert_eq!(
        restored.snapshot().unwrap().phase,
        AccountPhase::Disconnected
    );
    restored.close().unwrap();
}

#[test]
fn observers_do_not_retain_closed_scope_and_invalid_names_never_enter_custody() {
    let custody = Arc::new(Custody::default());
    let f = Fixture::new(custody.clone());
    let owner = AccountOwner::new(f.session().unwrap());
    let observations = owner.subscribe();
    assert!(
        owner.submit(AccountCommand::Snapshot).is_err(),
        "no runtime cannot accept work"
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        for name in ["".into(), "x".repeat(129), "private\nname".into()] {
            assert_eq!(
                owner
                    .submit(AccountCommand::Begin { name, now: 1 })
                    .err()
                    .unwrap()
                    .kind(),
                io::ErrorKind::InvalidInput
            );
        }
        owner
            .submit(AccountCommand::Close)
            .unwrap()
            .wait()
            .await
            .unwrap();
    });
    drop(owner);
    assert_eq!(*observations.borrow(), AccountObservation::Closed);
    assert!(custody.values.lock().unwrap().is_empty());
    let mut restored = f.session().unwrap();
    restored.close().unwrap();
}

#[test]
fn construction_retains_an_already_uncertain_session_for_explicit_recovery() {
    let custody = Arc::new(Custody::default());
    let f = Fixture::new(custody.clone());
    let mut session = f.session().unwrap();
    custody.fail.store(true, Ordering::SeqCst);
    assert!(session.begin("Desktop", 1).is_err());
    assert!(session.custody_pending());
    let owner = AccountOwner::new(session);
    let observations = owner.subscribe();
    assert_eq!(*observations.borrow(), AccountObservation::Uninspected);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        assert!(
            owner
                .submit(AccountCommand::Close)
                .unwrap()
                .wait()
                .await
                .is_err()
        );
        assert_eq!(
            *observations.borrow(),
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
        owner
            .submit(AccountCommand::Close)
            .unwrap()
            .wait()
            .await
            .unwrap();
    });
    assert_eq!(*observations.borrow(), AccountObservation::Closed);
    let mut restored = f.session().unwrap();
    assert_eq!(restored.snapshot().unwrap().phase, AccountPhase::Prepared);
    restored.close().unwrap();
}

#[test]
fn locked_custody_replaces_ready_observation_without_fallback_or_identity_reset() {
    let custody = Arc::new(Custody::default());
    let f = Fixture::new(custody.clone());
    let owner = AccountOwner::new(f.session().unwrap());
    let observations = owner.subscribe();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let AccountReply::Snapshot(original) = owner
            .submit(AccountCommand::Snapshot)
            .unwrap()
            .wait()
            .await
            .unwrap()
        else {
            panic!("snapshot")
        };
        assert!(matches!(
            *observations.borrow(),
            AccountObservation::Ready { .. }
        ));
        custody.locked.store(true, Ordering::SeqCst);
        let error = owner
            .submit(AccountCommand::Snapshot)
            .unwrap()
            .wait()
            .await
            .err()
            .unwrap();
        assert!(!error.to_string().contains("private-custody-detail"));
        assert_eq!(
            *observations.borrow(),
            AccountObservation::RecoveryRequired {
                custody_pending: false
            }
        );
        custody.locked.store(false, Ordering::SeqCst);
        let AccountReply::Snapshot(restored) = owner
            .submit(AccountCommand::Snapshot)
            .unwrap()
            .wait()
            .await
            .unwrap()
        else {
            panic!("snapshot")
        };
        assert_eq!(restored, original);
        assert!(custody.values.lock().unwrap().is_empty());
        owner
            .submit(AccountCommand::Close)
            .unwrap()
            .wait()
            .await
            .unwrap();
    });
}
