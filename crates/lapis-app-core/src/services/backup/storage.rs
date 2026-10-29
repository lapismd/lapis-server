use super::{Owner, failure, provider, watch};
use crate::{
    services::{Error, Result, VaultService},
    vault::VaultRoot,
};
use jj_vault::{Backup, Binding, NativeRepository, Storage, Timing};
use lapis_app_protocol::{ErrorCode, VaultPhase};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::sync::Mutex as AsyncMutex;

/// Installation-owned external history/control directory. It is never a DTO.
pub struct BackupStorage {
    root: VaultRoot,
    pub(super) files: Option<Arc<Mutex<()>>>,
    pub(super) history_indexing: bool,
}
impl BackupStorage {
    pub fn new(directory: &Path) -> Result<Self> {
        if !directory.is_absolute()
            || !std::fs::symlink_metadata(directory).is_ok_and(|entry| entry.is_dir())
        {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "History storage requires an existing physical directory",
            ));
        }
        Ok(Self {
            root: VaultRoot::open(directory)?,
            files: None,
            history_indexing: false,
        })
    }
    /// A native host's existing gate also covers its OS-specific file actions.
    pub fn with_files_gate(mut self, files: Arc<Mutex<()>>) -> Self {
        self.files = Some(files);
        self
    }
    /// Explicit native profile selection; the default admits no index worker.
    pub fn with_history_indexing(mut self, enabled: bool) -> Self {
        self.history_indexing = enabled;
        self
    }
    pub(crate) fn validate_vault(&self, vault: &VaultRoot) -> Result<()> {
        self.root.verify_physical_root()?;
        if self.root.path().starts_with(vault.path()) || vault.path().starts_with(self.root.path())
        {
            return Err(Error::denied());
        }
        Ok(())
    }
    pub(in crate::services) fn history_source(
        &self,
        vault: &VaultService,
    ) -> Result<jj_vault::search::NativeHistory> {
        let state = vault.lock()?;
        state.root.verify_physical_root()?;
        self.validate_vault(&state.root)?;
        let id = vault.id.to_string();
        let storage = Storage::new(
            Binding {
                vault_id: id.clone(),
                root: state.root.path().into(),
                state: self.root.path().join(&id),
            },
            self.root.path().join(".layouts").join(&id),
        );
        // Binding/status only read installed control records. In particular,
        // this path never runs recovery, initialization, capture or relink.
        if storage.status().map_err(|_| failure())?.pending.is_some() {
            return Err(Error::busy());
        }
        Ok(jj_vault::search::NativeHistory {
            binding: storage.binding().map_err(|_| failure())?,
            exclusions: vec![],
        })
    }
    pub(in crate::services) fn history_cache(
        &self,
        vault: &VaultService,
    ) -> Result<std::path::PathBuf> {
        self.root.verify_physical_root()?;
        self.validate_vault(&vault.lock()?.root)?;
        Ok(self
            .root
            .path()
            .join(".reader-cache")
            .join(vault.id.to_string()))
    }
    pub(super) fn start(&self, vault: &Arc<VaultService>) -> Result<Owner> {
        let root = {
            let state = vault.lock()?;
            if state.phase == VaultPhase::Closed {
                return Err(Error::closed());
            }
            state.root.verify_physical_root()?;
            self.validate_vault(&state.root)?;
            state.root.try_clone()?
        };
        let id = vault.id.to_string();
        let storage = Storage::new(
            Binding {
                vault_id: id.clone(),
                root: root.path().into(),
                state: self.root.path().join(&id),
            },
            self.root.path().join(".layouts").join(&id),
        );
        let executor = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| failure())?;
        executor
            .block_on(storage.recover())
            .map_err(|_| failure())?;
        let binding = storage.binding().map_err(|_| failure())?;
        self.validate_vault(&root)?;
        root.verify_physical_root()?;
        let engine = Backup::start_idle(
            provider::Provider {
                native: NativeRepository {
                    binding: binding.clone(),
                    exclusions: vec![],
                },
                vault: Arc::downgrade(vault),
            },
            Timing::default(),
        )
        .map_err(|_| failure())?;
        let watcher = watch::start(root.path(), engine.changes())?;
        executor.block_on(engine.capture()).map_err(|_| failure())?;
        if self.history_indexing {
            vault.ensure_history_indexing(Some(engine.subscribe()))?;
        }
        Ok(Owner {
            status: engine.subscribe(),
            engine: AsyncMutex::new(engine),
            watch: Mutex::new(Some(watcher)),
        })
    }
}
