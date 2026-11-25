use super::*;
use std::{fs, os::unix::fs::PermissionsExt as _};

const ACCOUNT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaab";
const VALUE: &str =
    r#"{"access_token":"synthetic-access","refresh_token":"synthetic-refresh","expires_at":null}"#;

#[test]
fn forge_credentials_keep_full_account_identity_and_survive_encrypted_restart() {
    let root = tempfile::tempdir().unwrap();
    let private = root.path().join("private");
    fs::create_dir(&private).unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700)).unwrap();
    let key = root.path().join("operator-key");
    fs::write(&key, [7u8; 32]).unwrap();
    let installation = Uuid::new_v4();
    let owner = Uuid::new_v4();
    let store = CredentialStore::open(&private, installation, Some(&key));
    store
        .forge_set(owner, ACCOUNT, Zeroizing::new(VALUE.into()))
        .unwrap();
    store
        .forge_set(owner, OTHER, Zeroizing::new(VALUE.into()))
        .unwrap();
    assert!(
        store.inventory(owner).unwrap().is_empty(),
        "forge account metadata has its own inventory"
    );
    let token = store
        .save(
            owner,
            TokenInput {
                label: "Independent repository token".into(),
                endpoint: "https://example.test/notes.git".into(),
                username: "fixture".into(),
                token: "synthetic-independent-token".into(),
            },
        )
        .unwrap();
    assert_eq!(store.inventory(owner).unwrap().len(), 1);
    assert_eq!(
        store.forge_get(owner, ACCOUNT).unwrap().unwrap().as_str(),
        VALUE
    );
    assert!(store.forge_get(Uuid::new_v4(), ACCOUNT).unwrap().is_none());
    for account in [ACCOUNT, OTHER] {
        let bytes =
            fs::read(private.join(store.forge_record_name(installation, owner, account))).unwrap();
        assert!(!bytes.windows(16).any(|v| v == b"synthetic-access"));
        assert!(!bytes.windows(17).any(|v| v == b"synthetic-refresh"));
    }
    drop(store);
    let store = CredentialStore::open(&private, installation, Some(&key));
    assert_eq!(
        store
            .resolve(owner, token.id, &token.endpoint)
            .unwrap()
            .token
            .as_str(),
        "synthetic-independent-token"
    );
    assert_eq!(
        store.forge_get(owner, ACCOUNT).unwrap().unwrap().as_str(),
        VALUE
    );
    let first = private.join(store.forge_record_name(installation, owner, ACCOUNT));
    let second = private.join(store.forge_record_name(installation, owner, OTHER));
    fs::write(second, fs::read(first).unwrap()).unwrap();
    assert!(
        store.forge_get(owner, OTHER).is_err(),
        "all 256 identity bits must be authenticated"
    );
    assert!(!store.available());
}

#[test]
fn forge_replacement_forgetting_and_locked_storage_never_fall_back_to_plaintext() {
    let root = tempfile::tempdir().unwrap();
    let private = root.path().join("private");
    fs::create_dir(&private).unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700)).unwrap();
    let key = root.path().join("operator-key");
    fs::write(&key, [7u8; 32]).unwrap();
    let installation = Uuid::new_v4();
    let owner = Uuid::new_v4();
    let store = CredentialStore::open(&private, installation, Some(&key));
    assert!(store.forge_get(owner, ACCOUNT).unwrap().is_none());
    store
        .forge_set(owner, ACCOUNT, Zeroizing::new(VALUE.into()))
        .unwrap();
    let replacement = VALUE.replace("synthetic-access", "synthetic-replaced");
    store
        .forge_set(owner, ACCOUNT, Zeroizing::new(replacement.clone()))
        .unwrap();
    assert_eq!(
        *store.forge_get(owner, ACCOUNT).unwrap().unwrap(),
        replacement
    );
    assert!(
        store
            .forge_set(owner, "invalid", Zeroizing::new(VALUE.into()))
            .is_err()
    );
    assert!(
        store
            .forge_set(Uuid::nil(), ACCOUNT, Zeroizing::new(VALUE.into()))
            .is_err()
    );
    drop(store);
    let locked = CredentialStore::open(&private, installation, None);
    assert!(locked.forge_get(owner, ACCOUNT).is_err());
    assert!(
        locked
            .forge_set(owner, ACCOUNT, Zeroizing::new(VALUE.into()))
            .is_err()
    );
    assert!(locked.forge_remove(owner, ACCOUNT).is_err());
    drop(locked);
    let store = CredentialStore::open(&private, installation, Some(&key));
    store.forge_remove(owner, ACCOUNT).unwrap();
    assert!(store.forge_get(owner, ACCOUNT).unwrap().is_none());
    drop(store);
    let store = CredentialStore::open(&private, installation, Some(&key));
    assert!(store.forge_get(owner, ACCOUNT).unwrap().is_none());
}

#[test]
fn lost_key_verifier_never_rekeys_existing_forge_ciphertext() {
    let root = tempfile::tempdir().unwrap();
    let private = root.path().join("private");
    fs::create_dir(&private).unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700)).unwrap();
    let key = root.path().join("operator-key");
    fs::write(&key, [7u8; 32]).unwrap();
    let installation = Uuid::new_v4();
    let owner = Uuid::new_v4();
    let store = CredentialStore::open(&private, installation, Some(&key));
    store
        .forge_set(owner, ACCOUNT, Zeroizing::new(VALUE.into()))
        .unwrap();
    drop(store);
    fs::remove_file(private.join(".key-check")).unwrap();
    let file = private.join(locked_record_name(installation, owner));
    let before = fs::read(&file).unwrap();
    fs::write(&key, [8u8; 32]).unwrap();
    let locked = CredentialStore::open(&private, installation, Some(&key));
    assert!(!locked.available());
    assert!(!private.join(".key-check").exists());
    assert_eq!(fs::read(file).unwrap(), before);
}

fn locked_record_name(installation: Uuid, owner: Uuid) -> String {
    CredentialStore(None).forge_record_name(installation, owner, ACCOUNT)
}
