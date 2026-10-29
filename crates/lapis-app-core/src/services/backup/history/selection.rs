use super::{HistoryIndexStatus, Owner, not_selected, unavailable};
use crate::services::{Result, VaultService};
use std::sync::{Arc, atomic::Ordering};
use tokio::sync::watch::Receiver;

impl VaultService {
    /// Admit derived reads of existing history without starting backup capture.
    /// Native hosts call on a blocking worker; no client owns the returned work.
    pub fn start_history_indexing(self: &Arc<Self>) -> Result<HistoryIndexStatus> {
        let _work = self.begin_work()?;
        let backup = self
            .backup
            .lock()
            .map_err(|_| unavailable())?
            .as_ref()
            .map(|owner| owner.status.clone());
        let owner = self.ensure_history_indexing(backup)?;
        let status = owner.status.lock().map_err(|_| unavailable())?.clone();
        Ok(status)
    }
    pub(in crate::services::backup) fn ensure_history_indexing(
        self: &Arc<Self>,
        backup: Option<Receiver<jj_vault::Status>>,
    ) -> Result<Arc<Owner>> {
        let storage = self
            .backup_storage
            .as_ref()
            .filter(|storage| storage.history_indexing)
            .ok_or_else(not_selected)?;
        storage.history_source(self)?;
        let mut slot = self.history_indexing.lock().map_err(|_| unavailable())?;
        if let Some(owner) = slot.as_ref() {
            if backup.is_some() {
                *owner.backup.lock().map_err(|_| unavailable())? = backup;
            }
            owner.requested.store(true, Ordering::Release);
            return Ok(owner.clone());
        }
        let owner = Arc::new(Owner::start(Arc::downgrade(self), backup)?);
        *slot = Some(owner.clone());
        Ok(owner)
    }
    /// Read cached progress only; never admit capture or index work.
    pub fn history_indexing_status(&self) -> Result<HistoryIndexStatus> {
        let state = self.lock()?;
        state.require_open()?;
        state.root.verify_physical_root()?;
        self.backup_storage
            .as_ref()
            .ok_or_else(not_selected)?
            .validate_vault(&state.root)?;
        drop(state);
        let slot = self.history_indexing.lock().map_err(|_| unavailable())?;
        let owner = slot.as_ref().ok_or_else(not_selected)?;
        let status = owner.status.lock().map_err(|_| unavailable())?.clone();
        Ok(status)
    }
    /// Feature disable is distinct from pausing automatic capture.
    pub fn stop_history_indexing(self: &Arc<Self>) -> Result<()> {
        let _work = self.begin_work()?;
        self.finish_history_indexing()
    }
    pub(in crate::services) fn finish_history_indexing(&self) -> Result<()> {
        let mut slot = self.history_indexing.lock().map_err(|_| unavailable())?;
        if let Some(owner) = slot.as_ref() {
            owner.finish()?;
        }
        *slot = None;
        Ok(())
    }
}
