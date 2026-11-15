use super::*;
use std::{fs, os::unix::fs::PermissionsExt as _, sync::Arc};

#[test]
fn injected_forge_store_binds_owner_and_never_uses_the_native_keyring() {
    let root = tempfile::tempdir().unwrap();
    let private = root.path().join("private");
    fs::create_dir(&private).unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700)).unwrap();
    let key = root.path().join("operator-key");
    fs::write(&key, [7u8; 32]).unwrap();
    let store = Arc::new(CredentialStore::open(&private, Uuid::new_v4(), Some(&key)));
    let first = store.forge_secrets(Uuid::new_v4()).unwrap();
    let second = store.forge_secrets(Uuid::new_v4()).unwrap();
    let account = "b".repeat(64);
    first.set(&account, "synthetic-opaque-forge-json").unwrap();
    assert_eq!(
        first.get(&account).unwrap().as_deref(),
        Some("synthetic-opaque-forge-json")
    );
    assert_eq!(second.get(&account).unwrap(), None);
    assert!(store.forge_secrets(Uuid::nil()).is_err());
    first.remove(&account).unwrap();
    assert_eq!(first.get(&account).unwrap(), None);
    first.remove(&account).unwrap();
}

#[test]
fn locked_custody_refuses_forge_injection_without_a_fallback_store() {
    let root = tempfile::tempdir().unwrap();
    let private = root.path().join("private");
    fs::create_dir(&private).unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700)).unwrap();
    let store = Arc::new(CredentialStore::open(&private, Uuid::new_v4(), None));
    assert!(store.forge_secrets(Uuid::new_v4()).is_err());
    assert!(fs::read_dir(private).unwrap().next().is_none());
}
