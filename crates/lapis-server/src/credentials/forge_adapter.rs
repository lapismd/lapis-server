//! An explicitly injected forge manager can only reach its authenticated owner.
use super::{CredentialStore, unavailable};
use jj_forge::{Error, Result, accounts::store::SecretStore};
use std::{io, sync::Arc};
use uuid::Uuid;
use zeroize::Zeroizing;

struct ForgeSecrets {
    store: Arc<CredentialStore>,
    owner: Uuid,
}
impl CredentialStore {
    /// Installation composition supplies the authenticated owner, never a DTO.
    /// A locked server cannot substitute jj-forge's native keyring default.
    pub fn forge_secrets(self: &Arc<Self>, owner: Uuid) -> io::Result<Arc<dyn SecretStore>> {
        if owner.is_nil() || !self.available() {
            return Err(unavailable());
        }
        Ok(Arc::new(ForgeSecrets {
            store: self.clone(),
            owner,
        }))
    }
}
impl SecretStore for ForgeSecrets {
    fn get(&self, id: &str) -> Result<Option<String>> {
        self.store
            .forge_get(self.owner, id)
            .map(|value| value.map(|v| v.to_string()))
            .map_err(|_| Error::State)
    }
    fn set(&self, id: &str, value: &str) -> Result<()> {
        self.store
            .forge_set(self.owner, id, Zeroizing::new(value.to_owned()))
            .map_err(|_| Error::State)
    }
    fn remove(&self, id: &str) -> Result<()> {
        self.store
            .forge_remove(self.owner, id)
            .map_err(|_| Error::State)
    }
}
