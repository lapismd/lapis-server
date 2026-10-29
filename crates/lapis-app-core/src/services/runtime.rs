use super::{DatabaseStorage, Error, Result, VaultService};
use crate::vault::VaultRoot;
use lapis_app_protocol::ErrorCode;
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Default)]
pub struct Runtime {
    registry: Mutex<Registry>,
    storage: Option<Arc<DatabaseStorage>>,
    tasks: Option<Arc<super::TaskStorage>>,
    backup: Option<Arc<super::BackupStorage>>,
    turns: Option<Arc<super::TurnStorage>>,
}
#[derive(Default)]
struct Registry {
    closing: bool,
    vaults: HashMap<Uuid, Arc<VaultService>>,
    roots: HashMap<(u64, u64), Uuid>,
}
impl Runtime {
    pub fn with_tasks(mut self, storage: super::TaskStorage) -> Self {
        self.tasks = Some(Arc::new(storage));
        self
    }
    pub fn with_turns(mut self, storage: super::TurnStorage) -> Self {
        self.turns = Some(Arc::new(storage));
        self
    }
    /// Select the existing backup engine using native installation storage.
    pub fn with_backup(mut self, storage: super::BackupStorage) -> Self {
        self.backup = Some(Arc::new(storage));
        self
    }
    /// Select shared database ownership using native installation policy.
    pub fn with_database(storage: DatabaseStorage) -> Self {
        Self {
            storage: Some(Arc::new(storage)),
            ..Self::default()
        }
    }
    pub fn admit(&self, id: Uuid, path: &Path) -> Result<Arc<VaultService>> {
        if id.is_nil() {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "Vault identity is required",
            ));
        }
        self.admit_root(id, VaultRoot::open(path)?)
    }
    /// Consume a host's retained physical capability without reopening its path.
    pub fn admit_root(&self, id: Uuid, root: VaultRoot) -> Result<Arc<VaultService>> {
        if id.is_nil() {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "Vault identity is required",
            ));
        }
        root.verify_physical_root()?;
        if let Some(storage) = &self.storage {
            storage.validate_vault(&root)?;
        }
        if let Some(storage) = &self.backup {
            storage.validate_vault(&root)?;
        }
        if let Some(storage) = &self.tasks {
            storage.validate_vault(&root)?;
        }
        if let Some(storage) = &self.turns {
            storage.validate_vault(&root)?;
        }
        let identity = root.physical_identity()?;
        let mut registry = self.registry.lock().map_err(|_| Error::closed())?;
        if registry.closing {
            return Err(Error::closed());
        }
        if let Some(existing) = registry.vaults.get(&id) {
            let state = existing.lock()?;
            state.require_open()?;
            state.root.verify_physical_root()?;
            if state.root.physical_identity()? != identity {
                return Err(Error::denied());
            }
            return Ok(existing.clone());
        }
        if registry.roots.contains_key(&identity) {
            return Err(Error::busy());
        }
        let mut vault = VaultService::new(id, root, self.storage.clone());
        vault.backup_storage = self.backup.clone();
        vault.turn_storage = self.turns.clone();
        vault.task_storage = self.tasks.clone();
        let vault = Arc::new(vault);
        registry.roots.insert(identity, id);
        registry.vaults.insert(id, vault.clone());
        Ok(vault)
    }
    pub fn get(&self, id: Uuid) -> Result<Arc<VaultService>> {
        let registry = self.registry.lock().map_err(|_| Error::closed())?;
        if registry.closing {
            return Err(Error::closed());
        }
        registry.vaults.get(&id).cloned().ok_or_else(Error::denied)
    }
    pub fn close_vault(&self, id: Uuid) -> Result<()> {
        let vault = self.get(id)?;
        vault.close()?;
        let mut registry = self.registry.lock().map_err(|_| Error::closed())?;
        if registry
            .vaults
            .get(&id)
            .is_some_and(|current| Arc::ptr_eq(current, &vault))
        {
            registry.vaults.remove(&id);
            registry.roots.retain(|_, owner| *owner != id);
        }
        Ok(())
    }
    pub fn shutdown(&self) -> Result<()> {
        let mut registry = self.registry.lock().map_err(|_| Error::closed())?;
        let vaults: Vec<_> = registry.vaults.values().cloned().collect();
        // Hold every vault state through preflight: a failed leave keeps all owners live.
        let mut states = Vec::new();
        for vault in &vaults {
            states.push(vault.lock()?);
        }
        if states.iter().any(|state| state.editors_blocked(None)) {
            return Err(Error::busy());
        }
        registry.closing = true;
        for state in &mut states {
            if self.backup.is_some() && state.phase != lapis_app_protocol::VaultPhase::Closed {
                // Keep database writers usable if final capture needs repair.
                state.backup_pending = true;
                state.phase = lapis_app_protocol::VaultPhase::Closing;
            } else {
                state.begin_close();
            }
        }
        drop(states);
        drop(registry);
        for vault in vaults {
            vault.close()?;
        }
        let mut registry = self.registry.lock().map_err(|_| Error::closed())?;
        registry.vaults.clear();
        registry.roots.clear();
        Ok(())
    }
}
