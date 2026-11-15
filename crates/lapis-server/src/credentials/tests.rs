use super::*;
use std::fs;
use tempfile::TempDir;

const TOKEN: &str = "synthetic-repository-token-for-custody-tests";
struct Fixture {
    temp: TempDir,
    directory: std::path::PathBuf,
    key: std::path::PathBuf,
    installation: Uuid,
    owner: Uuid,
}
impl Fixture {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt as _;
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("private");
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let key = temp.path().join("operator-key");
        fs::write(&key, [7u8; 32]).unwrap();
        Self {
            temp,
            directory,
            key,
            installation: Uuid::new_v4(),
            owner: Uuid::new_v4(),
        }
    }
    fn open(&self) -> CredentialStore {
        CredentialStore::open(&self.directory, self.installation, Some(&self.key))
    }
    fn input(&self) -> TokenInput {
        TokenInput {
            label: "Fixture token".into(),
            endpoint: "https://example.test/team/notes.git".into(),
            username: "fixture".into(),
            token: TOKEN.into(),
        }
    }
    fn bytes(&self) -> Vec<(String, Vec<u8>)> {
        let mut files: Vec<_> = fs::read_dir(&self.directory)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (
                    entry.file_name().to_str().unwrap().into(),
                    fs::read(entry.path()).unwrap(),
                )
            })
            .collect();
        files.sort();
        files
    }
}
#[test]
fn tokens_survive_restart_without_entering_inventory_or_plaintext_files() {
    let f = Fixture::new();
    let store = f.open();
    assert!(store.available());
    let saved = store.save(f.owner, f.input()).unwrap();
    let inventory = store.inventory(f.owner).unwrap();
    assert_eq!(inventory.len(), 1);
    assert!(!serde_json::to_string(&inventory).unwrap().contains(TOKEN));
    for (_, bytes) in f.bytes() {
        assert!(
            !bytes
                .windows(TOKEN.len())
                .any(|part| part == TOKEN.as_bytes())
        );
    }
    drop(store);
    let reopened = f.open();
    assert!(reopened.available());
    let secret = reopened
        .resolve(f.owner, saved.id, &saved.endpoint)
        .unwrap();
    assert_eq!(secret.username, "fixture");
    assert_eq!(secret.token.as_str(), TOKEN);
}
#[test]
fn replacement_and_forgetting_persist_and_preserve_the_opaque_reference() {
    let f = Fixture::new();
    let store = f.open();
    let saved = store.save(f.owner, f.input()).unwrap();
    store
        .replace(
            f.owner,
            saved.id,
            Zeroizing::new("synthetic-replacement".into()),
        )
        .unwrap();
    drop(store);
    let store = f.open();
    assert_eq!(store.inventory(f.owner).unwrap()[0].id, saved.id);
    assert_eq!(
        store
            .resolve(f.owner, saved.id, &saved.endpoint)
            .unwrap()
            .token
            .as_str(),
        "synthetic-replacement"
    );
    store.forget(f.owner, saved.id).unwrap();
    drop(store);
    let store = f.open();
    assert!(store.inventory(f.owner).unwrap().is_empty());
    assert!(store.resolve(f.owner, saved.id, &saved.endpoint).is_err());
}
#[test]
fn missing_wrong_and_invalid_keys_lock_without_rewriting_ciphertext() {
    let f = Fixture::new();
    let store = f.open();
    store.save(f.owner, f.input()).unwrap();
    drop(store);
    let before = f.bytes();
    assert!(!CredentialStore::open(&f.directory, f.installation, None).available());
    for key in [vec![4u8; 32], vec![7u8; 31], vec![7u8; 33]] {
        fs::write(&f.key, key).unwrap();
        let locked = f.open();
        assert!(!locked.available());
        assert!(locked.inventory(f.owner).is_err());
        assert!(locked.save(f.owner, f.input()).is_err());
        assert_eq!(f.bytes(), before);
    }
}
#[test]
fn installation_owner_identity_and_endpoint_scopes_are_bound() {
    let f = Fixture::new();
    let store = f.open();
    let saved = store.save(f.owner, f.input()).unwrap();
    assert!(store.inventory(Uuid::new_v4()).unwrap().is_empty());
    assert!(store.inventory(Uuid::nil()).is_err());
    assert!(
        store
            .resolve(Uuid::new_v4(), saved.id, &saved.endpoint)
            .is_err()
    );
    for endpoint in [
        "https://example.test/another.git",
        "https://other.test/team/notes.git",
        "http://example.test/team/notes.git",
    ] {
        assert!(store.resolve(f.owner, saved.id, endpoint).is_err());
    }
    assert!(store.available());
    drop(store);
    assert!(!CredentialStore::open(&f.directory, Uuid::new_v4(), Some(&f.key)).available());
}
#[test]
fn swapped_record_id_and_modified_ciphertext_lock_the_store() {
    let f = Fixture::new();
    let store = f.open();
    let first = store.save(f.owner, f.input()).unwrap();
    let second = store.save(f.owner, f.input()).unwrap();
    let first_path = f.directory.join(format!("{}-{}.sealed", f.owner, first.id));
    let second_path = f
        .directory
        .join(format!("{}-{}.sealed", f.owner, second.id));
    fs::write(&second_path, fs::read(&first_path).unwrap()).unwrap();
    assert!(store.resolve(f.owner, second.id, &second.endpoint).is_err());
    assert!(!store.available());
    drop(store);
    let store = f.open();
    let mut bytes = fs::read(&first_path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    fs::write(first_path, bytes).unwrap();
    assert!(store.resolve(f.owner, first.id, &first.endpoint).is_err());
    assert!(!store.available());
}
#[test]
fn only_one_native_custodian_opens_a_private_store() {
    let f = Fixture::new();
    let first = f.open();
    assert!(first.available());
    assert!(!f.open().available());
    drop(first);
    assert!(f.open().available());
}
#[test]
fn no_verifier_or_invalid_secret_never_creates_a_plaintext_fallback() {
    let f = Fixture::new();
    let store = f.open();
    store.save(f.owner, f.input()).unwrap();
    drop(store);
    fs::remove_file(f.directory.join(".key-check")).unwrap();
    assert!(!f.open().available());
    assert!(!f.directory.join(".key-check").exists());
    let other = Fixture::new();
    let store = other.open();
    let mut input = other.input();
    input.token = "synthetic\nheader".into();
    assert!(store.save(other.owner, input).is_err());
    assert!(store.inventory(other.owner).unwrap().is_empty());
    assert!(other.temp.path().join("operator-key").exists());
}

#[test]
fn owner_rebinding_is_authenticated_and_keys_cannot_live_beside_ciphertext() {
    let f = Fixture::new();
    let store = f.open();
    let saved = store.save(f.owner, f.input()).unwrap();
    let other = Uuid::new_v4();
    fs::rename(
        f.directory.join(format!("{}-{}.sealed", f.owner, saved.id)),
        f.directory.join(format!("{other}-{}.sealed", saved.id)),
    )
    .unwrap();
    assert!(store.resolve(other, saved.id, &saved.endpoint).is_err());
    assert!(!store.available());
    drop(store);
    let misplaced_key = f.directory.join("operator-key");
    fs::copy(&f.key, &misplaced_key).unwrap();
    assert!(!CredentialStore::open(&f.directory, f.installation, Some(&misplaced_key)).available());
}

#[test]
fn concurrent_saves_share_one_bounded_native_writer() {
    let f = Fixture::new();
    let store = f.open();
    for _ in 0..LIMIT - 1 {
        store.save(f.owner, f.input()).unwrap();
    }
    let accepted = std::thread::scope(|scope| {
        let tasks: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| store.save(f.owner, f.input()).is_ok()))
            .collect();
        tasks
            .into_iter()
            .map(|task| task.join().unwrap())
            .filter(|accepted| *accepted)
            .count()
    });
    assert_eq!(accepted, 1);
    assert_eq!(store.inventory(f.owner).unwrap().len(), LIMIT);
}

#[test]
fn explicitly_approved_origin_scope_retains_its_account_and_refuses_other_hosts() {
    let f = Fixture::new();
    let store = f.open();
    let mut input = f.input();
    input.endpoint = "https://example.test".into();
    let saved = store.save(f.owner, input).unwrap();
    for endpoint in [
        "https://example.test/team/one.git",
        "https://example.test/team/two.git",
    ] {
        assert_eq!(
            store.resolve(f.owner, saved.id, endpoint).unwrap().username,
            "fixture"
        );
    }
    for endpoint in [
        "https://other.test/team/one.git",
        "https://example.test:8443/team/one.git",
        "http://example.test/team/one.git",
    ] {
        assert!(store.resolve(f.owner, saved.id, endpoint).is_err());
    }
}
