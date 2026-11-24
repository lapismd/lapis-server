use super::{CredentialStore, Unlocked, unavailable};
use lapis_app_core::credentials::{
    CredentialError, MAX_SECRET_BYTES, ScopedCredentialStore, SecretBytes, SecretName,
};
use sha2::{Digest as _, Sha256};
use std::{
    io,
    ops::Deref,
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

pub(super) struct Scope<S> {
    pub(super) store: S,
    pub(super) owner: Uuid,
    pub(super) namespace: CredentialNamespace,
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

pub(super) struct Record {
    pub(super) name: String,
    pub(super) aad: Vec<u8>,
    pub(super) limit: usize,
}

impl<S> Scope<S> {
    pub(super) fn record(&self, installation: Uuid, name: &SecretName) -> Record {
        let mut aad = b"lapis.credential.scope.v1\0".to_vec();
        aad.extend_from_slice(installation.as_bytes());
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
            limit: MAX_ENVELOPE,
        }
    }
}

impl Record {
    pub(super) fn read(
        &self,
        store: &Unlocked,
    ) -> Result<Option<(Zeroizing<Vec<u8>>, Vec<u8>)>, CredentialError> {
        let metadata = match store.root.entry_metadata(&self.name) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(CredentialError::Unavailable),
        };
        if !metadata.is_file() || metadata.len() > self.limit as u64 {
            store.locked.store(true, Ordering::Release);
            return Err(CredentialError::Locked);
        }
        let ciphertext = store
            .root
            .read_range(&self.name, 0, self.limit + 1)
            .map_err(|_| CredentialError::Unavailable)?;
        let secret = store.cipher.open_aad(&self.aad, &ciphertext).map_err(|_| {
            store.locked.store(true, Ordering::Release);
            CredentialError::Locked
        })?;
        Ok(Some((secret, ciphertext)))
    }

    pub(super) fn write(
        &self,
        store: &Unlocked,
        plaintext: &[u8],
        before: Option<&[u8]>,
    ) -> Result<(), CredentialError> {
        if plaintext.len() + 29 > self.limit {
            return Err(CredentialError::InvalidInput);
        }
        let bytes = store
            .cipher
            .seal_aad(&self.aad, plaintext)
            .map_err(|_| CredentialError::Unavailable)?;
        store
            .root
            .write_if_unchanged(&self.name, &bytes[..], before, &Default::default())
            .map_err(|_| CredentialError::Unavailable)?;
        store.verify().map_err(|_| CredentialError::Unavailable)
    }

    pub(super) fn remove(&self, store: &Unlocked) -> Result<(), CredentialError> {
        store
            .root
            .remove_file(&self.name)
            .map_err(|_| CredentialError::Unavailable)?;
        store.verify().map_err(|_| CredentialError::Unavailable)
    }
}

impl<S: Deref<Target = CredentialStore> + Send + Sync> ScopedCredentialStore for Scope<S> {
    fn get(&self, name: &SecretName) -> Result<Option<SecretBytes>, CredentialError> {
        let (store, _guard) = self.store.ready().map_err(|_| CredentialError::Locked)?;
        self.record(store.cipher.installation(), name)
            .read(store)?
            .map(|(secret, _)| SecretBytes::new(secret.to_vec()))
            .transpose()
    }

    fn set(&self, name: &SecretName, value: &SecretBytes) -> Result<(), CredentialError> {
        let (store, _guard) = self.store.ready().map_err(|_| CredentialError::Locked)?;
        let record = self.record(store.cipher.installation(), name);
        let before = record.read(store)?;
        if before.is_none()
            && store
                .scoped_count()
                .map_err(|_| CredentialError::Unavailable)?
                >= MAX_RECORDS
        {
            return Err(CredentialError::Unavailable);
        }
        record.write(
            store,
            value.expose_secret(),
            before.as_ref().map(|(_, b)| b.as_slice()),
        )
    }

    fn delete(&self, name: &SecretName) -> Result<(), CredentialError> {
        let (store, _guard) = self.store.ready().map_err(|_| CredentialError::Locked)?;
        let record = self.record(store.cipher.installation(), name);
        if record.read(store)?.is_some() {
            record.remove(store)?;
        }
        store.verify().map_err(|_| CredentialError::Unavailable)
    }
}
