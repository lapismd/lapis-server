use lapis_app_core::{
    sync::account::{
        AccountEndpoint, AccountTransportPolicy,
        protected::{HostAccountCustody, HostSecretBytes, ProtectedAccountState},
        session::{AccountPhase, AccountSession},
    },
    vault::VaultRoot,
};
use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Default)]
struct Custody {
    values: Mutex<HashMap<String, Vec<u8>>>,
    failure: Mutex<u8>,
    writes: Mutex<Vec<Vec<u8>>>,
}
impl HostAccountCustody for Custody {
    fn supported(&self) -> bool {
        true
    }
    fn read(&self, key: &str) -> io::Result<Option<HostSecretBytes>> {
        self.values
            .lock()
            .unwrap()
            .get(key)
            .map(|v| HostSecretBytes::new(v.clone()))
            .transpose()
    }
    fn write(&self, key: &str, value: &HostSecretBytes) -> io::Result<()> {
        self.writes
            .lock()
            .unwrap()
            .push(value.expose_secret().to_vec());
        let failure = *self.failure.lock().unwrap();
        if failure != 1 {
            self.values
                .lock()
                .unwrap()
                .insert(key.into(), value.expose_secret().to_vec());
        }
        if failure != 0 {
            Err(io::Error::other("synthetic-secret-custody-failure"))
        } else {
            Ok(())
        }
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
    fn endpoint(origin: &str) -> AccountEndpoint {
        AccountEndpoint::open(origin, AccountTransportPolicy::Https).unwrap()
    }
    fn store(&self) -> ProtectedAccountState {
        ProtectedAccountState::open(
            &self.profile,
            &self.vault,
            self.profile_id,
            self.vault_id,
            &Self::endpoint("https://sync.example"),
            self.custody.clone(),
        )
        .unwrap()
    }
    fn session(&self) -> AccountSession {
        AccountSession::new(self.store(), Self::endpoint("https://sync.example"))
    }
}
#[test]
fn uncertain_preparation_fences_all_network_actions_until_identical_custody_retry() {
    for mode in [1, 2] {
        let f = Fixture::new();
        let mut session = f.session();
        let client = session.snapshot().unwrap().client_id;
        *f.custody.failure.lock().unwrap() = mode;
        assert!(session.begin("Desktop", 1).is_err());
        assert!(session.custody_pending());
        assert!(session.snapshot().is_err());
        assert!(session.begin("Another request", 1).is_err());
        assert!(session.continue_prepared(1).is_err());
        assert!(session.redeem(1).is_err());
        assert!(session.disconnect(1).is_err());
        assert!(session.close().is_err());
        *f.custody.failure.lock().unwrap() = 0;
        assert_eq!(
            session.retry_custody().unwrap().phase,
            AccountPhase::Prepared
        );
        let writes = f.custody.writes.lock().unwrap();
        assert_eq!(writes.len(), if mode == 1 { 2 } else { 1 });
        if mode == 1 {
            assert_eq!(writes[0], writes[1]);
        }
        drop(writes);
        assert_eq!(
            session.abandon_unissued(1).unwrap().phase,
            AccountPhase::Disconnected
        );
        session.close().unwrap();
        let mut restored = f.session();
        assert_eq!(restored.snapshot().unwrap().client_id, client);
        assert_eq!(
            restored.snapshot().unwrap().phase,
            AccountPhase::Disconnected
        );
        restored.close().unwrap();
    }
}
#[test]
fn foreign_endpoint_and_invalid_input_fail_before_custody_or_transport_mutation() {
    let f = Fixture::new();
    let store = f.store();
    let client = store.client_id();
    let mut wrong = AccountSession::new(store, Fixture::endpoint("https://other.example"));
    assert!(wrong.snapshot().is_err());
    assert!(wrong.begin("Desktop", 1).is_err());
    assert!(wrong.disconnect(1).is_err());
    assert!(f.custody.writes.lock().unwrap().is_empty());
    wrong.close().unwrap();
    let mut original = f.session();
    assert_eq!(original.snapshot().unwrap().client_id, client);
    for name in ["", "\n", &"x".repeat(129)] {
        assert!(original.begin(name, 1).is_err());
    }
    assert!(original.begin("Desktop", -1).is_err());
    assert!(original.continue_prepared(1).is_err());
    assert!(original.redeem(1).is_err());
    assert!(original.discover(1).is_err());
    assert!(original.inspect().is_err());
    assert!(f.custody.writes.lock().unwrap().is_empty());
    original.close().unwrap();
}
#[test]
fn corrupt_or_foreign_session_payloads_never_restore_as_disconnected() {
    for payload in [
        b"not-json-private-proof".to_vec(),
        br#"{"version":999,"phase":{"phase":"disconnected"}}"#.to_vec(),
    ] {
        let f = Fixture::new();
        let mut store = f.store();
        let client = store.client_id();
        store
            .write(&HostSecretBytes::new(payload.clone()).unwrap())
            .unwrap();
        store.close().unwrap();
        let mut session = f.session();
        let error = session.snapshot().unwrap_err();
        assert!(!error.to_string().contains("private-proof"));
        assert!(session.begin("Replacement", 1).is_err());
        assert!(session.abandon_unissued(i64::MAX).is_err());
        assert_eq!(f.custody.writes.lock().unwrap().len(), 1);
        session.close().unwrap();
        assert_eq!(f.store().client_id(), client);
    }
}

#[test]
fn protected_record_identity_version_and_phase_fields_are_validated() {
    for case in 0..6 {
        let f = Fixture::new();
        let mut store = f.store();
        let client = store.client_id();
        let mut record = serde_json::json!({"version":1,"origin":"https://sync.example","client":client,"phase":{"phase":"disconnected"}});
        match case {
            0 => record["version"] = serde_json::json!(2),
            1 => record["origin"] = serde_json::json!("https://foreign.example"),
            2 => record["client"] = serde_json::json!(Uuid::new_v4()),
            3 => record["phase"]["phase"] = serde_json::json!("unrecognized"),
            4 => {
                record["phase"] = serde_json::json!({"phase":"prepared","name":"Desktop","proof":"invalid-private-proof"})
            }
            _ => record["phase"]["unknown"] = serde_json::json!("private-proof"),
        }
        store
            .write(&HostSecretBytes::new(serde_json::to_vec(&record).unwrap()).unwrap())
            .unwrap();
        store.close().unwrap();
        let mut session = f.session();
        assert!(session.snapshot().is_err(), "case {case}");
        assert!(session.begin("Replacement", 1).is_err());
        assert_eq!(f.custody.writes.lock().unwrap().len(), 1);
        session.close().unwrap();
    }
}
