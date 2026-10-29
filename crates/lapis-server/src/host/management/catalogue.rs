use crate::host::{Result, error};
use lapis_app_core::vault::VaultRoot;
use lapis_app_protocol::{ErrorCode, ManagedVault};
use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard},
};
use uuid::Uuid;

pub(super) struct Entry {
    pub vault: ManagedVault,
    pub root: VaultRoot,
    pub available: bool,
}
#[derive(Default)]
pub(in crate::host) struct Catalogue(Mutex<BTreeMap<Uuid, Entry>>);

pub(in crate::host) struct Admission<'a> {
    _registry: MutexGuard<'a, BTreeMap<Uuid, Entry>>,
}

impl Catalogue {
    pub fn list(&self) -> Result<Vec<ManagedVault>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| failed())?
            .values()
            .map(|entry| entry.vault.clone())
            .collect())
    }
    pub(super) fn insert(&self, entry: Entry) -> Result<()> {
        self.0
            .lock()
            .map_err(|_| failed())?
            .insert(entry.vault.vault_id, entry);
        Ok(())
    }
    pub fn get(&self, id: Uuid) -> Result<ManagedVault> {
        self.0
            .lock()
            .map_err(|_| failed())?
            .get(&id)
            .map(|entry| entry.vault.clone())
            .ok_or_else(|| error(ErrorCode::NotFound, "Vault is unavailable"))
    }
    pub fn root(&self, id: Uuid) -> Result<VaultRoot> {
        self.0
            .lock()
            .map_err(|_| failed())?
            .get(&id)
            .ok_or_else(|| error(ErrorCode::NotFound, "Vault is unavailable"))?
            .root
            .try_clone()
            .map_err(|_| failed())
    }
    pub fn admission(&self, id: Uuid) -> Result<Admission<'_>> {
        let guard = self.0.lock().map_err(|_| failed())?;
        let entry = guard
            .get(&id)
            .ok_or_else(|| error(ErrorCode::PermissionDenied, "Vault is unavailable"))?;
        if entry.vault.archived_at.is_some() {
            return Err(error(ErrorCode::PermissionDenied, "Vault is archived"));
        }
        if !entry.available {
            return Err(error(
                ErrorCode::Busy,
                "Vault lifecycle is pending; retry the operation",
            ));
        }
        Ok(Admission { _registry: guard })
    }
    pub fn available(&self, id: Uuid, value: bool) -> Result<()> {
        self.0
            .lock()
            .map_err(|_| failed())?
            .get_mut(&id)
            .ok_or_else(|| error(ErrorCode::NotFound, "Vault is unavailable"))?
            .available = value;
        Ok(())
    }
    pub fn publish(&self, vault: ManagedVault) -> Result<()> {
        let mut guard = self.0.lock().map_err(|_| failed())?;
        let entry = guard.get_mut(&vault.vault_id).ok_or_else(failed)?;
        entry.available = vault.archived_at.is_none();
        entry.vault = vault;
        Ok(())
    }
}
fn failed() -> lapis_app_protocol::ServiceError {
    error(ErrorCode::Io, "Vault catalogue is unavailable")
}
