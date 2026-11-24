use super::{CredentialStore, Unlocked, unavailable};
use lapis_app_core::credentials::{
    CredentialError, MAX_SECRET_BYTES, ScopedCredentialStore, SecretBytes, SecretName,
};
use sha2::{Digest as _, Sha256};
use std::{
    io,
    sync::{Arc, atomic::Ordering},
};
use uuid::Uuid;
use zeroize::Zeroizing;

pub(super) const MAX_RECORDS: usize = 4096;
const PREFIX: &str = ".scoped-";
const MAX_ENVELOPE: usize = MAX_SECRET_BYTES + 29;

pub enum CredentialNamespace {
    Plugin(SecretName),
    RepositoryForge,
}

struct Scope {
    store: Arc<CredentialStore>,
    owner: Uuid,
    namespace: CredentialNamespace,
}

impl CredentialStore {
    pub fn scope(
        self: &Arc<Self>,
        owner: Uuid,
        namespace: CredentialNamespace,
    ) -> Result<Arc<dyn ScopedCredentialStore>, CredentialError> {
        if owner.is_nil() {
            return Err(CredentialError::InvalidInput);
        }
        Ok(Arc::new(Scope {
            store: self.clone(),
            owner,
            namespace,
        }))
    }
}

pub(super) fn is_record_name(name: &str) -> bool {
    name.strip_prefix(PREFIX).is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

impl Unlocked {
    pub(super) fn scoped_count(&self) -> io::Result<usize> {
        self.verify()?;
        let mut count = 0;
        for (index, entry) in self.root.directory_handle()?.entries()?.enumerate() {
            if index >= MAX_RECORDS + super::LIMIT + 4 {
                return Err(unavailable());
            }
            let entry = entry?;
            let name = entry.file_name().into_string().map_err(|_| unavailable())?;
            if is_record_name(&name) {
                if !entry.file_type()?.is_file() || count >= MAX_RECORDS {
                    return Err(unavailable());
                }
                count += 1;
            }
        }
        Ok(count)
    }
}

struct Record {
    name: String,
    aad: Vec<u8>,
}

impl Scope {
    fn record(&self, store: &Unlocked, name: &SecretName) -> Record {
        let mut aad = b"lapis.credential.scope.v1\0".to_vec();
        aad.extend_from_slice(store.cipher.installation().as_bytes());
        aad.extend_from_slice(self.owner.as_bytes());
        match &self.namespace {
            CredentialNamespace::RepositoryForge => aad.push(0),
            CredentialNamespace::Plugin(id) => {
                aad.push(1);
                let id = id.as_str().as_bytes();
                aad.extend_from_slice(&(id.len() as u16).to_be_bytes());
                aad.extend_from_slice(id);
            }
        }
        aad.extend_from_slice(name.as_str().as_bytes());
        Record {
            name: format!("{PREFIX}{:x}", Sha256::digest(&aad)),
            aad,
        }
    }
}

impl Record {
    fn read(
        &self,
        store: &Unlocked,
    ) -> Result<Option<(Zeroizing<Vec<u8>>, Vec<u8>)>, CredentialError> {
        let metadata = match store.root.entry_metadata(&self.name) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(CredentialError::Unavailable),
        };
        if !metadata.is_file() || metadata.len() > MAX_ENVELOPE as u64 {
            return Err(CredentialError::Locked);
        }
        let ciphertext = store
            .root
            .read_range(&self.name, 0, MAX_ENVELOPE + 1)
            .map_err(|_| CredentialError::Unavailable)?;
        let secret = store.cipher.open_aad(&self.aad, &ciphertext).map_err(|_| {
            store.locked.store(true, Ordering::Release);
            CredentialError::Locked
        })?;
        Ok(Some((secret, ciphertext)))
    }
}

impl ScopedCredentialStore for Scope {
    fn get(&self, name: &SecretName) -> Result<Option<SecretBytes>, CredentialError> {
        let (store, _guard) = self.store.ready().map_err(|_| CredentialError::Locked)?;
        self.record(store, name)
            .read(store)?
            .map(|(secret, _)| SecretBytes::new(secret.to_vec()))
            .transpose()
    }

    fn set(&self, name: &SecretName, value: &SecretBytes) -> Result<(), CredentialError> {
        let (store, _guard) = self.store.ready().map_err(|_| CredentialError::Locked)?;
        let record = self.record(store, name);
        let before = record.read(store)?;
        if before.is_none()
            && store
                .scoped_count()
                .map_err(|_| CredentialError::Unavailable)?
                >= MAX_RECORDS
        {
            return Err(CredentialError::Unavailable);
        }
        let bytes = store
            .cipher
            .seal_aad(&record.aad, value.expose_secret())
            .map_err(|_| CredentialError::Unavailable)?;
        store
            .root
            .write_if_unchanged(
                &record.name,
                &bytes[..],
                before.as_ref().map(|(_, b)| b.as_slice()),
                &Default::default(),
            )
            .map_err(|_| CredentialError::Unavailable)?;
        store.verify().map_err(|_| CredentialError::Unavailable)
    }

    fn delete(&self, name: &SecretName) -> Result<(), CredentialError> {
        let (store, _guard) = self.store.ready().map_err(|_| CredentialError::Locked)?;
        let record = self.record(store, name);
        if record.read(store)?.is_some() {
            store
                .root
                .remove_file(&record.name)
                .map_err(|_| CredentialError::Unavailable)?;
        }
        store.verify().map_err(|_| CredentialError::Unavailable)
    }
}
