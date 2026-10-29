use super::source::Source;
use crate::services::{VaultService, Work};
use jj_history_service::{Admission, Context, Error, Result, local};
use jj_store_model::RepositoryId;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, Weak},
};
use uuid::Uuid;

pub(super) const ORIGIN: &str = "lapis-core://repository";
#[derive(Clone)]
pub(super) struct Policy {
    pub(super) vault: Weak<VaultService>,
    grants: Arc<Mutex<HashMap<Uuid, Source>>>,
    pub(super) originals: Arc<Mutex<HashMap<Uuid, super::original::Grant>>>,
}
impl Policy {
    pub fn new(vault: Weak<VaultService>) -> Self {
        Self {
            vault,
            grants: Arc::default(),
            originals: Arc::default(),
        }
    }
    pub fn vault(&self, client: Uuid, requested: &str) -> Result<Arc<VaultService>> {
        let vault = self.vault.upgrade().ok_or(Error::Denied)?;
        if requested != vault.id.to_string() {
            let grants = self.originals.lock().map_err(|_| Error::Unavailable)?;
            let grant = grants
                .get(&client)
                .filter(|grant| grant.token == requested)
                .ok_or(Error::Denied)?;
            if grant.revoked {
                return Err(Error::EpochChanged);
            }
        }
        vault
            .lock()
            .map_err(|_| Error::Unavailable)?
            .require_client(client)
            .map_err(|_| Error::Denied)?;
        Ok(vault)
    }
    pub fn source(vault: &VaultService) -> Result<Option<Source>> {
        let native = vault
            .backup_storage
            .as_ref()
            .ok_or(Error::Denied)?
            .history_source(vault)
            .map_err(|_| Error::Denied)?;
        let physical = vault
            .lock()
            .map_err(|_| Error::Unavailable)?
            .root
            .physical_identity()
            .map_err(|_| Error::Denied)?;
        Source::existing(&native.binding, physical).map_err(|_| Error::Denied)
    }
    pub fn identity(vault: &VaultService, original: Option<&Source>) -> (RepositoryId, u64) {
        let prefix: &[u8] = if original.is_some() {
            b"lapis-original-repository-v1\0"
        } else {
            b"lapis-managed-repository-v1\0"
        };
        let mut hash = Sha256::new_with_prefix(prefix);
        hash.update(vault.id.as_bytes());
        let digest: [u8; 32] = hash.finalize().into();
        let generation = if let Some(source) = original {
            let mut epoch = Sha256::new_with_prefix(b"lapis-original-epoch-v1\0");
            epoch.update(vault.generation.as_bytes());
            epoch.update(source.fingerprint());
            let digest: [u8; 32] = epoch.finalize().into();
            u64::from_be_bytes(digest[..8].try_into().unwrap()).max(1)
        } else {
            u64::from_be_bytes(vault.generation.as_bytes()[..8].try_into().unwrap()).max(1)
        };
        (
            RepositoryId::new(digest[..16].try_into().unwrap()),
            generation,
        )
    }
    pub fn revoke(&self, client: Uuid) -> Result<()> {
        self.originals
            .lock()
            .map_err(|_| Error::Unavailable)?
            .remove(&client);
        self.grants
            .lock()
            .map_err(|_| Error::Unavailable)?
            .remove(&client);
        Ok(())
    }
}
struct Access {
    policy: Policy,
    client: Uuid,
    vault: String,
    source: Source,
    identity: (RepositoryId, u64),
    _work: Work,
}
#[async_trait::async_trait]
impl Admission for Policy {
    async fn admit(&self, caller: &Context, requested: &str) -> Result<Arc<dyn local::Access>> {
        if caller.url != ORIGIN {
            return Err(Error::Denied);
        }
        let client = Uuid::parse_str(&caller.window).map_err(|_| Error::Denied)?;
        let policy = self.clone();
        let requested = requested.to_string();
        tokio::task::spawn_blocking(move || {
            let vault = policy.vault(client, &requested)?;
            let work = vault.begin_work().map_err(|_| Error::Denied)?;
            let original = requested != vault.id.to_string();
            let source = policy
                .resolve(client, &requested, &vault)?
                .ok_or(Error::Unavailable)?;
            if !original {
                let mut grants = policy.grants.lock().map_err(|_| Error::Unavailable)?;
                if let Some(previous) = grants.get(&client) {
                    if previous != &source {
                        return Err(Error::EpochChanged);
                    }
                } else {
                    if grants.len() >= 16 {
                        return Err(Error::Busy);
                    }
                    grants.insert(client, source.clone());
                }
            }
            Ok(Arc::new(Access {
                identity: Self::identity(&vault, original.then_some(&source)),
                policy,
                client,
                vault: requested,
                source,
                _work: work,
            }) as Arc<dyn local::Access>)
        })
        .await
        .map_err(|_| Error::Unavailable)?
    }
}
#[async_trait::async_trait]
impl local::Access for Access {
    fn repository(&self) -> RepositoryId {
        self.identity.0
    }
    fn generation(&self) -> u64 {
        self.identity.1
    }
    fn metadata_path(&self) -> &Path {
        &self.source.metadata
    }
    async fn authorize(&self) -> Result<()> {
        let policy = self.policy.clone();
        let client = self.client;
        let requested = self.vault.clone();
        let expected = self.source.clone();
        tokio::task::spawn_blocking(move || {
            let vault = policy.vault(client, &requested)?;
            if policy.resolve(client, &requested, &vault)?.as_ref() != Some(&expected) {
                return Err(Error::EpochChanged);
            }
            if expected.verify().is_err() {
                if requested != vault.id.to_string() {
                    policy.revoke_original(client)?;
                }
                return Err(Error::EpochChanged);
            }
            policy.vault(client, &requested)?;
            Ok(())
        })
        .await
        .map_err(|_| Error::Unavailable)?
    }
}
