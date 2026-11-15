use lapis_app_core::{
    sync::account::{
        AccountEndpoint, AccountTransportPolicy,
        protected::{HostAccountCustody, HostSecretBytes, ProtectedAccountState},
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
    override_read: Mutex<Option<Vec<u8>>>,
}
impl HostAccountCustody for Custody {
    fn supported(&self) -> bool {
        true
    }
    fn read(&self, key: &str) -> io::Result<Option<HostSecretBytes>> {
        if *self.failure.lock().unwrap() == 3 {
            return Err(io::Error::other("synthetic-secret-provider-locked"));
        }
        if let Some(value) = &*self.override_read.lock().unwrap() {
            return HostSecretBytes::new(value.clone()).map(Some);
        }
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
            return Err(io::Error::other("synthetic-secret-provider-error"));
        }
        Ok(())
    }
}
struct Fixture {
    _temp: tempfile::TempDir,
    profile: VaultRoot,
    vault: VaultRoot,
    profile_id: Uuid,
    vault_id: Uuid,
    endpoint: AccountEndpoint,
    custody: Arc<Custody>,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("profile")).unwrap();
        std::fs::create_dir(temp.path().join("vault")).unwrap();
        Self {
            profile: VaultRoot::open(&temp.path().join("profile")).unwrap(),
            vault: VaultRoot::open(&temp.path().join("vault")).unwrap(),
            profile_id: Uuid::new_v4(),
            vault_id: Uuid::new_v4(),
            endpoint: AccountEndpoint::open("https://sync.example", AccountTransportPolicy::Https)
                .unwrap(),
            custody: Arc::new(Custody::default()),
            _temp: temp,
        }
    }
    fn open(&self) -> io::Result<ProtectedAccountState> {
        ProtectedAccountState::open(
            &self.profile,
            &self.vault,
            self.profile_id,
            self.vault_id,
            &self.endpoint,
            self.custody.clone(),
        )
    }
}
#[test]
fn protected_account_identity_and_payload_survive_reopen_without_plaintext_files() {
    let f = Fixture::new();
    let mut state = f.open().unwrap();
    let client = state.client_id();
    assert!(state.read().unwrap().is_none());
    let secret =
        HostSecretBytes::new(b"retained-private-account-proof-and-bearer".to_vec()).unwrap();
    state.write(&secret).unwrap();
    assert_eq!(
        state.read().unwrap().unwrap().expose_secret(),
        secret.expose_secret()
    );
    state.close().unwrap();
    drop(state);
    let mut reopened = f.open().unwrap();
    assert_eq!(reopened.client_id(), client);
    assert_eq!(
        reopened.read().unwrap().unwrap().expose_secret(),
        secret.expose_secret()
    );
    fn verify_files(path: &std::path::Path, secret: &[u8]) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                verify_files(&entry.path(), secret);
            } else {
                assert!(
                    !std::fs::read(entry.path())
                        .unwrap()
                        .windows(secret.len())
                        .any(|v| v == secret)
                );
            }
        }
    }
    verify_files(f.profile.path(), secret.expose_secret());
    assert!(std::fs::read_dir(f.vault.path()).unwrap().next().is_none());
    reopened.close().unwrap();
}
#[test]
fn uncertain_protected_writes_block_close_and_inspect_before_reusing_identical_bytes() {
    for mode in [1, 2] {
        let f = Fixture::new();
        let mut state = f.open().unwrap();
        *f.custody.failure.lock().unwrap() = mode;
        let secret = HostSecretBytes::new(b"uncertain-credential".to_vec()).unwrap();
        let error = state.write(&secret).unwrap_err();
        assert!(!error.to_string().contains("synthetic-secret"));
        assert!(state.has_pending_write());
        assert!(f.open().is_err());
        assert_eq!(state.close().unwrap_err().kind(), io::ErrorKind::WouldBlock);
        assert!(state.read().is_err());
        *f.custody.failure.lock().unwrap() = 0;
        state.retry_pending().unwrap();
        assert!(!state.has_pending_write());
        let writes = f.custody.writes.lock().unwrap();
        assert_eq!(writes.len(), if mode == 1 { 2 } else { 1 });
        if mode == 1 {
            assert_eq!(writes[0], writes[1]);
        }
        drop(writes);
        assert_eq!(
            state.read().unwrap().unwrap().expose_secret(),
            secret.expose_secret()
        );
        state.close().unwrap();
    }
}
#[test]
fn exclusive_owner_is_released_only_after_an_explicit_successful_close() {
    let f = Fixture::new();
    let mut first = f.open().unwrap();
    let client = first.client_id();
    assert!(f.open().is_err());
    first.close().unwrap();
    let mut next = f.open().unwrap();
    assert_eq!(next.client_id(), client);
    next.close().unwrap();
}
#[test]
fn replaced_roots_and_missing_custody_refuse_without_replacing_identity() {
    let f = Fixture::new();
    let mut state = f.open().unwrap();
    let client = state.client_id();
    state
        .write(&HostSecretBytes::new(b"credential".to_vec()).unwrap())
        .unwrap();
    f.custody.values.lock().unwrap().clear();
    assert_eq!(state.read().err().unwrap().kind(), io::ErrorKind::NotFound);
    assert!(
        state
            .write(&HostSecretBytes::new(b"replacement".to_vec()).unwrap())
            .is_err()
    );
    state.close().unwrap();
    let mut reopened = f.open().unwrap();
    assert_eq!(reopened.client_id(), client);
    assert!(reopened.read().is_err());
    let old = f.vault.path().with_file_name("old-vault");
    std::fs::rename(f.vault.path(), old).unwrap();
    std::fs::create_dir(f.vault.path()).unwrap();
    assert!(reopened.read().is_err());
    reopened.close().unwrap();
}

#[test]
fn unsupported_locked_and_oversized_custody_never_select_another_store() {
    struct Unsupported;
    impl HostAccountCustody for Unsupported {
        fn supported(&self) -> bool {
            false
        }
        fn read(&self, _: &str) -> io::Result<Option<HostSecretBytes>> {
            panic!("Unsupported custody must not be called")
        }
        fn write(&self, _: &str, _: &HostSecretBytes) -> io::Result<()> {
            panic!("Unsupported custody must not be called")
        }
    }
    let f = Fixture::new();
    assert_eq!(
        ProtectedAccountState::open(
            &f.profile,
            &f.vault,
            f.profile_id,
            f.vault_id,
            &f.endpoint,
            Arc::new(Unsupported)
        )
        .err()
        .unwrap()
        .kind(),
        io::ErrorKind::Unsupported
    );
    assert!(!f.profile.path().join("sync").exists());
    let mut state = f.open().unwrap();
    let client = state.client_id();
    *f.custody.failure.lock().unwrap() = 3;
    assert!(state.read().is_err());
    assert!(
        state
            .write(&HostSecretBytes::new(b"new-value".to_vec()).unwrap())
            .is_err()
    );
    assert!(f.custody.writes.lock().unwrap().is_empty());
    *f.custody.failure.lock().unwrap() = 0;
    assert!(HostSecretBytes::new(vec![0; 32 * 1024 + 1]).is_err());
    assert!(
        state
            .write(&HostSecretBytes::new(vec![0; 16 * 1024 + 1]).unwrap())
            .is_err()
    );
    assert_eq!(state.client_id(), client);
    assert!(f.custody.writes.lock().unwrap().is_empty());
    state.close().unwrap();
}
#[test]
fn foreign_protected_envelopes_and_replaced_profile_admission_are_refused() {
    let f = Fixture::new();
    let mut state = f.open().unwrap();
    let client = state.client_id();
    state
        .write(&HostSecretBytes::new(b"original-secret".to_vec()).unwrap())
        .unwrap();
    state.close().unwrap();
    let foreign = f
        .custody
        .values
        .lock()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .clone();
    *f.custody.override_read.lock().unwrap() = Some(foreign);
    let other =
        AccountEndpoint::open("https://other.example", AccountTransportPolicy::Https).unwrap();
    for (profile_id, vault_id, endpoint) in [
        (Uuid::new_v4(), f.vault_id, &f.endpoint),
        (f.profile_id, Uuid::new_v4(), &f.endpoint),
        (f.profile_id, f.vault_id, &other),
    ] {
        assert!(
            ProtectedAccountState::open(
                &f.profile,
                &f.vault,
                profile_id,
                vault_id,
                endpoint,
                f.custody.clone()
            )
            .is_err()
        );
    }
    *f.custody.override_read.lock().unwrap() = None;
    let mut restored = f.open().unwrap();
    assert_eq!(restored.client_id(), client);
    assert_eq!(f.custody.writes.lock().unwrap().len(), 1);
    restored.close().unwrap();
    std::fs::rename(
        f.profile.path(),
        f.profile.path().with_file_name("old-profile"),
    )
    .unwrap();
    std::fs::create_dir(f.profile.path()).unwrap();
    assert!(f.open().is_err());
    assert!(
        std::fs::read_dir(f.profile.path())
            .unwrap()
            .next()
            .is_none()
    );
}
#[test]
fn changed_metadata_and_replaced_lock_file_fence_both_existing_and_new_owners() {
    let f = Fixture::new();
    let mut state = f.open().unwrap();
    fn find(path: &std::path::Path, name: &str) -> std::path::PathBuf {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name() == name {
                return entry.path();
            }
            if entry.file_type().unwrap().is_dir() {
                let found = find(&entry.path(), name);
                if !found.as_os_str().is_empty() {
                    return found;
                }
            }
        }
        std::path::PathBuf::new()
    }
    let marker = find(f.profile.path(), "binding.json");
    let original = std::fs::read(&marker).unwrap();
    std::fs::write(&marker, b"{}").unwrap();
    assert!(state.read().is_err());
    std::fs::write(&marker, &original).unwrap();
    let lock = find(f.profile.path(), ".owner.lock");
    let old = lock.with_file_name("retained-owner.lock");
    std::fs::rename(&lock, &old).unwrap();
    std::fs::write(&lock, b"").unwrap();
    assert!(state.read().is_err());
    assert!(f.open().is_err());
    std::fs::remove_file(&lock).unwrap();
    std::fs::rename(old, lock).unwrap();
    assert!(state.read().unwrap().is_none());
    state.close().unwrap();
}

#[test]
fn process_owner_lock_and_public_identity_survive_a_cold_process() {
    const ROOT: &str = "LAPIS_SYNC_ACCOUNT_OWNER_TEST_ROOT";
    if let Some(root) = std::env::var_os(ROOT) {
        let root = std::path::PathBuf::from(root);
        std::env::set_current_dir(std::env::temp_dir()).unwrap();
        let profile = VaultRoot::open(&root.join("profile")).unwrap();
        let vault = VaultRoot::open(&root.join("vault")).unwrap();
        let id = |key| Uuid::parse_str(&std::env::var(key).unwrap()).unwrap();
        let endpoint =
            AccountEndpoint::open("https://sync.example", AccountTransportPolicy::Https).unwrap();
        let opened = ProtectedAccountState::open(
            &profile,
            &vault,
            id("LAPIS_SYNC_ACCOUNT_OWNER_TEST_PROFILE"),
            id("LAPIS_SYNC_ACCOUNT_OWNER_TEST_VAULT"),
            &endpoint,
            Arc::new(Custody::default()),
        );
        if std::env::var("LAPIS_SYNC_ACCOUNT_OWNER_TEST_EXPECT_OPEN").unwrap() == "yes" {
            let mut state = opened.unwrap();
            assert_eq!(
                state.client_id(),
                id("LAPIS_SYNC_ACCOUNT_OWNER_TEST_CLIENT")
            );
            assert!(state.read().unwrap().is_none());
            state.close().unwrap();
        } else {
            assert!(opened.is_err());
        }
        return;
    }
    let f = Fixture::new();
    let mut owner = f.open().unwrap();
    let client = owner.client_id();
    let child = |expected| {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "process_owner_lock_and_public_identity_survive_a_cold_process",
                "--nocapture",
            ])
            .env(ROOT, f._temp.path().canonicalize().unwrap())
            .env(
                "LAPIS_SYNC_ACCOUNT_OWNER_TEST_PROFILE",
                f.profile_id.to_string(),
            )
            .env(
                "LAPIS_SYNC_ACCOUNT_OWNER_TEST_VAULT",
                f.vault_id.to_string(),
            )
            .env("LAPIS_SYNC_ACCOUNT_OWNER_TEST_CLIENT", client.to_string())
            .env("LAPIS_SYNC_ACCOUNT_OWNER_TEST_EXPECT_OPEN", expected)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "cold owner test failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    child("no");
    owner.close().unwrap();
    child("yes");
}

#[test]
fn pending_retry_refuses_foreign_custody_without_overwriting_evidence() {
    let f = Fixture::new();
    let mut state = f.open().unwrap();
    state
        .write(&HostSecretBytes::new(b"previous".to_vec()).unwrap())
        .unwrap();
    *f.custody.failure.lock().unwrap() = 1;
    assert!(
        state
            .write(&HostSecretBytes::new(b"desired".to_vec()).unwrap())
            .is_err()
    );
    *f.custody.failure.lock().unwrap() = 0;
    *f.custody.override_read.lock().unwrap() = Some(b"foreign-custody-evidence".to_vec());
    let before = f.custody.writes.lock().unwrap().clone();
    assert_eq!(
        state.retry_pending().unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert!(state.has_pending_write());
    assert_eq!(*f.custody.writes.lock().unwrap(), before);
    assert!(state.close().is_err());
    *f.custody.override_read.lock().unwrap() = None;
    state.retry_pending().unwrap();
    let writes = f.custody.writes.lock().unwrap();
    assert_eq!(writes.len(), 3);
    assert_eq!(writes[1], writes[2]);
    drop(writes);
    assert_eq!(state.read().unwrap().unwrap().expose_secret(), b"desired");
    state.close().unwrap();
}

#[test]
fn missing_binding_and_custody_do_not_reinitialize_an_existing_scope() {
    let f = Fixture::new();
    let mut state = f.open().unwrap();
    state
        .write(&HostSecretBytes::new(b"recoverable-account".to_vec()).unwrap())
        .unwrap();
    state.close().unwrap();
    let key = f
        .custody
        .values
        .lock()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    let destination = key.strip_prefix("lapis.sync.account.v1.").unwrap();
    let binding = f
        .profile
        .path()
        .join("sync/v1/vaults")
        .join(f.vault_id.to_string())
        .join("destinations")
        .join(destination)
        .join("account/binding.json");
    std::fs::remove_file(&binding).unwrap();
    f.custody.values.lock().unwrap().clear();
    assert!(
        f.open().is_err(),
        "Missing established state requires recovery"
    );
    assert!(!binding.exists(), "Do not persist a replacement identity");
    assert_eq!(f.custody.writes.lock().unwrap().len(), 1);
}
