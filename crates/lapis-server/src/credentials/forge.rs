//! Adapts forge account identities to the shared encrypted store.
use super::{CredentialNamespace, CredentialStore, scoped::Scope, unavailable};
use lapis_app_core::credentials::{ScopedCredentialStore, SecretBytes, SecretName};
use std::io;
use uuid::Uuid;
use zeroize::Zeroizing;

const MAX_BYTES: usize = 12 * 1024;

fn account_name(account: &str) -> io::Result<SecretName> {
    if account.len() != 64
        || !account
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(unavailable());
    }
    SecretName::new(account).map_err(io::Error::other)
}

impl CredentialStore {
    fn forge_scope(&self, owner: Uuid) -> io::Result<Scope<&Self>> {
        if owner.is_nil() {
            return Err(unavailable());
        }
        Ok(Scope {
            store: self,
            owner,
            namespace: CredentialNamespace::RepositoryForge,
        })
    }

    pub fn forge_get(&self, owner: Uuid, account: &str) -> io::Result<Option<Zeroizing<String>>> {
        self.forge_scope(owner)?
            .get(&account_name(account)?)
            .map_err(io::Error::other)?
            .as_ref()
            .map(|value| {
                if value.expose_secret().is_empty() || value.expose_secret().len() > MAX_BYTES {
                    return Err(unavailable());
                }
                let text = std::str::from_utf8(value.expose_secret()).map_err(|_| unavailable())?;
                Ok(Zeroizing::new(text.to_owned()))
            })
            .transpose()
    }

    pub fn forge_set(
        &self,
        owner: Uuid,
        account: &str,
        value: Zeroizing<String>,
    ) -> io::Result<()> {
        if value.is_empty() || value.len() > MAX_BYTES {
            return Err(unavailable());
        }
        let value = SecretBytes::new(value.as_bytes().to_vec()).map_err(io::Error::other)?;
        self.forge_scope(owner)?
            .set(&account_name(account)?, &value)
            .map_err(io::Error::other)
    }

    pub fn forge_remove(&self, owner: Uuid, account: &str) -> io::Result<()> {
        self.forge_scope(owner)?
            .delete(&account_name(account)?)
            .map_err(io::Error::other)
    }

    #[cfg(test)]
    pub(super) fn forge_record_name(
        &self,
        installation: Uuid,
        owner: Uuid,
        account: &str,
    ) -> String {
        self.forge_scope(owner)
            .unwrap()
            .record(installation, &account_name(account).unwrap())
            .name
    }
}
