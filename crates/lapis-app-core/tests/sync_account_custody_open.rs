use lapis_app_core::{
    sync::account::{
        AccountEndpoint, AccountTransportPolicy,
        protected::{HostAccountCustody, HostSecretBytes, ProtectedAccountState},
    },
    vault::VaultRoot,
};
use std::{
    io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use uuid::Uuid;

struct Custody {
    profile: PathBuf,
    vault: Uuid,
    locked: AtomicBool,
    observed: Mutex<Vec<Option<serde_json::Value>>>,
    writes: AtomicUsize,
}
impl HostAccountCustody for Custody {
    fn supported(&self) -> bool {
        true
    }
    fn read(&self, key: &str) -> io::Result<Option<HostSecretBytes>> {
        let destination = key.strip_prefix("lapis.sync.account.v1.").unwrap();
        let marker = self
            .profile
            .join("sync/v1/vaults")
            .join(self.vault.to_string())
            .join("destinations")
            .join(destination)
            .join("account/binding.json");
        let value = std::fs::read(marker)
            .ok()
            .map(|bytes| serde_json::from_slice(&bytes).unwrap());
        self.observed.lock().unwrap().push(value);
        if self.locked.load(Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "private-custody-locked",
            ));
        }
        Ok(None)
    }
    fn write(&self, _: &str, _: &HostSecretBytes) -> io::Result<()> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        panic!("opening a disconnected account must not write a credential")
    }
}
fn open(
    profile: &Path,
    vault: &Path,
    profile_id: Uuid,
    vault_id: Uuid,
    custody: Arc<Custody>,
) -> io::Result<ProtectedAccountState> {
    ProtectedAccountState::open(
        &VaultRoot::open(profile)?,
        &VaultRoot::open(vault)?,
        profile_id,
        vault_id,
        &AccountEndpoint::open("https://sync.example", AccountTransportPolicy::Https)?,
        custody,
    )
}

#[test]
fn first_locked_custody_open_retries_after_unlock_without_replacing_identity() {
    const PROFILE: &str = "LAPIS_SYNC_OPEN_TEST_PROFILE";
    const VAULT: &str = "LAPIS_SYNC_OPEN_TEST_VAULT";
    const PROFILE_ID: &str = "LAPIS_SYNC_OPEN_TEST_PROFILE_ID";
    const VAULT_ID: &str = "LAPIS_SYNC_OPEN_TEST_VAULT_ID";
    const CLIENT: &str = "LAPIS_SYNC_OPEN_TEST_CLIENT";
    if let Some(profile) = std::env::var_os(PROFILE) {
        let vault = PathBuf::from(std::env::var_os(VAULT).unwrap());
        let vault_id = Uuid::parse_str(&std::env::var(VAULT_ID).unwrap()).unwrap();
        let custody = Arc::new(Custody {
            profile: PathBuf::from(&profile),
            vault: vault_id,
            locked: AtomicBool::new(false),
            observed: Mutex::new(vec![]),
            writes: AtomicUsize::new(0),
        });
        let state = open(
            Path::new(&profile),
            &vault,
            Uuid::parse_str(&std::env::var(PROFILE_ID).unwrap()).unwrap(),
            vault_id,
            custody.clone(),
        )
        .unwrap();
        assert_eq!(
            state.client_id().to_string(),
            std::env::var(CLIENT).unwrap()
        );
        assert!(state.read().unwrap().is_none());
        assert_eq!(custody.writes.load(Ordering::SeqCst), 0);
        return;
    }
    let profile = tempfile::tempdir().unwrap();
    let vault = tempfile::tempdir().unwrap();
    let profile_id = Uuid::new_v4();
    let vault_id = Uuid::new_v4();
    let custody = Arc::new(Custody {
        profile: profile.path().into(),
        vault: vault_id,
        locked: AtomicBool::new(true),
        observed: Mutex::new(vec![]),
        writes: AtomicUsize::new(0),
    });
    let error = open(
        profile.path(),
        vault.path(),
        profile_id,
        vault_id,
        custody.clone(),
    )
    .err()
    .unwrap();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(!error.to_string().contains("private-custody-locked"));
    custody.locked.store(false, Ordering::SeqCst);
    let mut restored = open(
        profile.path(),
        vault.path(),
        profile_id,
        vault_id,
        custody.clone(),
    )
    .expect("unlock must recover the original native scope instead of stranding its owner lock");
    let client = restored.client_id();
    let observations = custody.observed.lock().unwrap();
    let original = observations[0]
        .as_ref()
        .expect("reserve the public identity before custody IO");
    assert_eq!(original["scope"]["client"], client.to_string());
    assert_eq!(original["protected"], false);
    assert_eq!(observations[1].as_ref().unwrap(), original);
    drop(observations);
    restored.close().unwrap();
    drop(restored);
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "first_locked_custody_open_retries_after_unlock_without_replacing_identity",
            "--nocapture",
        ])
        .env(PROFILE, profile.path())
        .env(VAULT, vault.path())
        .env(PROFILE_ID, profile_id.to_string())
        .env(VAULT_ID, vault_id.to_string())
        .env(CLIENT, client.to_string())
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "cold child: {}",
        String::from_utf8_lossy(&child.stderr)
    );
    assert_eq!(custody.writes.load(Ordering::SeqCst), 0);
}
