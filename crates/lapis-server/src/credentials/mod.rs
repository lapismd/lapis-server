//! Installation-private custody. HTTP adapters supply authenticated owner IDs;
//! only native composition can choose this directory and the operator key file.
mod forge;
mod forge_adapter;
#[cfg(test)]
mod forge_adapter_tests;
#[cfg(test)]
mod forge_tests;
mod key;
mod record;
mod remotes;
#[cfg(test)]
mod remotes_tests;
mod scoped;
mod sealed;
#[cfg(test)]
mod tests;
use cap_std::fs::MetadataExt as _;
pub use key::{CredentialKey, KEY_ENV, KEY_FILE_ENV};
use lapis_app_core::vault::{VaultRoot, WriteOptions};
pub use record::{CredentialSummary, TokenInput};
pub use scoped::CredentialNamespace;
use sealed::{Cipher, unavailable};
use std::{
    fs::File,
    io,
    path::Path,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

const CHECK: &[u8] = b"lapis.repository.credentials.v1";
const LIMIT: usize = 128;
/// Stored secrets are never serializable or printable through this type.
pub struct NativeCredential {
    pub username: String,
    pub token: Zeroizing<String>,
}
pub struct CredentialStore(Option<Unlocked>);
struct Unlocked {
    root: VaultRoot,
    cipher: Cipher,
    lock: File,
    locked: AtomicBool,
    operations: Mutex<()>,
}
impl CredentialStore {
    /// The caller creates an installation-private 0700 directory outside vaults.
    /// Missing/incorrect keys refuse the entire store, including empty inventory.
    pub fn open(directory: &Path, installation: Uuid, key: Option<&Path>) -> Self {
        let key = key.and_then(|path| {
            if path
                .canonicalize()
                .ok()?
                .starts_with(directory.canonicalize().ok()?)
            {
                return None;
            }
            CredentialKey::from_file(path)
        });
        Self::open_with_key(directory, installation, key.as_ref())
    }
    pub fn open_with_key(
        directory: &Path,
        installation: Uuid,
        key: Option<&CredentialKey>,
    ) -> Self {
        Self(Unlocked::open(directory, installation, key).ok())
    }
    pub fn available(&self) -> bool {
        self.ready().is_ok()
    }
    fn ready(&self) -> io::Result<(&Unlocked, MutexGuard<'_, ()>)> {
        let store = self.0.as_ref().ok_or_else(unavailable)?;
        let guard = store.operations.lock().map_err(|_| unavailable())?;
        store.verify()?;
        let bytes = store.bytes(".key-check")?;
        if store
            .cipher
            .open(Uuid::nil(), Uuid::nil(), &bytes)?
            .as_slice()
            != CHECK
        {
            return Err(unavailable());
        }
        Ok((store, guard))
    }
    pub fn inventory(&self, owner: Uuid) -> io::Result<Vec<CredentialSummary>> {
        if owner.is_nil() {
            return Err(unavailable());
        }
        let (store, _guard) = self.ready()?;
        store
            .ids()?
            .into_iter()
            .filter(|(found, _)| *found == owner)
            .map(|(_, id)| {
                store
                    .read(owner, id)
                    .map(|(record, _)| record.summary.clone())
            })
            .collect()
    }
    pub fn save(&self, owner: Uuid, input: TokenInput) -> io::Result<CredentialSummary> {
        let (store, _guard) = self.ready()?;
        if owner.is_nil() || store.ids()?.len() >= LIMIT {
            return Err(unavailable());
        }
        let id = Uuid::new_v4();
        let record = record::Record::new(&input, id)?;
        store.write(owner, id, &record, None)?;
        Ok(record.summary.clone())
    }
    pub fn replace(&self, owner: Uuid, id: Uuid, token: Zeroizing<String>) -> io::Result<()> {
        record::token(&token)?;
        let (store, _guard) = self.ready()?;
        let (mut record, before) = store.read(owner, id)?;
        record.token.zeroize();
        record.token = token.to_string();
        store.write(owner, id, &record, Some(&before))
    }
    pub fn forget(&self, owner: Uuid, id: Uuid) -> io::Result<()> {
        let (store, _guard) = self.ready()?;
        // Decrypt before removing, so a swapped or foreign record is not accepted.
        store.read(owner, id)?;
        store
            .root
            .remove_file(&format!("{owner}-{id}.sealed"))
            .map_err(|_| unavailable())
    }
    pub fn resolve(&self, owner: Uuid, id: Uuid, endpoint: &str) -> io::Result<NativeCredential> {
        let (store, _guard) = self.ready()?;
        let (record, _) = store.read(owner, id)?;
        if !record::matches_scope(&record.summary.endpoint, endpoint)? {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Credential scope does not match",
            ));
        }
        Ok(NativeCredential {
            username: record.summary.username.clone(),
            token: Zeroizing::new(record.token.clone()),
        })
    }
}
impl Unlocked {
    fn open(directory: &Path, installation: Uuid, key: Option<&CredentialKey>) -> io::Result<Self> {
        let cipher = Cipher::new(key.ok_or_else(unavailable)?, installation)?;
        let root = VaultRoot::open(directory).map_err(|_| unavailable())?;
        if root.directory_handle()?.dir_metadata()?.mode() & 0o077 != 0 {
            return Err(unavailable());
        }
        match root.create_file(".custody.lock", &b""[..], &WriteOptions::default()) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(unavailable()),
        }
        let lock = root.directory_handle()?.open(".custody.lock")?.into_std();
        lock.try_lock().map_err(|_| unavailable())?;
        let store = Self {
            root,
            cipher,
            lock,
            locked: AtomicBool::new(false),
            operations: Mutex::new(()),
        };
        if store.root.stat(".key-check")?.is_none() {
            // Existing records with no verifier never authorize a fresh key.
            if !store.ids()?.is_empty() || store.has_forge_records()? || store.scoped_count()? != 0
            {
                return Err(unavailable());
            }
            let bytes = store.cipher.seal(Uuid::nil(), Uuid::nil(), CHECK)?;
            store
                .root
                .create_file(".key-check", &bytes[..], &WriteOptions::default())?;
        }
        let check = store.bytes(".key-check")?;
        if store
            .cipher
            .open(Uuid::nil(), Uuid::nil(), &check)?
            .as_slice()
            != CHECK
        {
            return Err(unavailable());
        }
        store.verify()?;
        Ok(store)
    }
    fn verify(&self) -> io::Result<()> {
        if self.locked.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        self.root
            .verify_physical_root()
            .map_err(|_| unavailable())?;
        use std::os::unix::fs::MetadataExt as _;
        let current = self.root.entry_metadata(".custody.lock")?;
        let held = self.lock.metadata()?;
        if !current.is_file() || current.dev() != held.dev() || current.ino() != held.ino() {
            return Err(unavailable());
        }
        Ok(())
    }
    fn ids(&self) -> io::Result<Vec<(Uuid, Uuid)>> {
        self.verify()?;
        let mut result = Vec::new();
        for (index, entry) in self.root.directory_handle()?.entries()?.enumerate() {
            if index >= LIMIT + scoped::MAX_RECORDS + 4 {
                return Err(unavailable());
            }
            let entry = entry?;
            let name = entry.file_name().into_string().map_err(|_| unavailable())?;
            if name == ".custody.lock" || name == ".key-check" || name.starts_with(".lapis-stage-")
            {
                continue;
            }
            if name == ".forge" && entry.file_type()?.is_dir() {
                continue;
            }
            if scoped::is_record_name(&name) && entry.file_type()?.is_file() {
                continue;
            }
            let ids = name.strip_suffix(".sealed").ok_or_else(unavailable)?;
            if ids.len() != 73 || !ids.is_ascii() || !entry.file_type()?.is_file() {
                return Err(unavailable());
            }
            let owner = Uuid::parse_str(&ids[..36]).map_err(|_| unavailable())?;
            let id = Uuid::parse_str(&ids[37..]).map_err(|_| unavailable())?;
            if owner.is_nil() || id.is_nil() || name != format!("{owner}-{id}.sealed") {
                return Err(unavailable());
            }
            result.push((owner, id));
            if result.len() > LIMIT {
                return Err(unavailable());
            }
        }
        result.sort();
        Ok(result)
    }
    fn bytes(&self, name: &str) -> io::Result<Vec<u8>> {
        let metadata = self.root.entry_metadata(name).map_err(|_| unavailable())?;
        if !metadata.is_file() || metadata.len() > 16 * 1024 {
            return Err(unavailable());
        }
        self.root
            .read_range(name, 0, 16 * 1024 + 1)
            .map_err(|_| unavailable())
    }
    fn read(&self, owner: Uuid, id: Uuid) -> io::Result<(record::Record, Vec<u8>)> {
        if owner.is_nil() || id.is_nil() {
            return Err(unavailable());
        }
        self.verify()?;
        let bytes = self.bytes(&format!("{owner}-{id}.sealed"))?;
        let plain = self.cipher.open(owner, id, &bytes).inspect_err(|_| {
            self.locked.store(true, Ordering::Release);
        })?;
        let record: record::Record = serde_json::from_slice(&plain).map_err(|_| unavailable())?;
        if record.summary.id != id
            || record::endpoint(&record.summary.endpoint)? != record.summary.endpoint
        {
            return Err(unavailable());
        }
        record::token(&record.token)?;
        Ok((record, bytes))
    }
    fn write(
        &self,
        owner: Uuid,
        id: Uuid,
        record: &record::Record,
        before: Option<&[u8]>,
    ) -> io::Result<()> {
        self.verify()?;
        let plain = Zeroizing::new(serde_json::to_vec(record).map_err(|_| unavailable())?);
        let bytes = self.cipher.seal(owner, id, &plain)?;
        let name = format!("{owner}-{id}.sealed");
        match before {
            Some(before) => self.root.write_if_unchanged(
                &name,
                &bytes[..],
                Some(before),
                &WriteOptions::default(),
            ),
            None => self
                .root
                .create_file(&name, &bytes[..], &WriteOptions::default()),
        }
        .map_err(|_| unavailable())?;
        self.verify()
    }
}
impl Drop for Unlocked {
    fn drop(&mut self) {
        let _ = self.lock.unlock();
    }
}
