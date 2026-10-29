use super::{Error, Result, VaultService, operations};
use lapis_app_protocol::{ExpectedVersion, TextSnapshot, VaultPhase};
use std::sync::Arc;

/// Native ownership lease. Ending it is not proof that a durable job succeeded.
pub struct Work {
    pub(super) vault: Arc<VaultService>,
}
impl VaultService {
    pub fn begin_work(self: &Arc<Self>) -> Result<Work> {
        let mut state = self.lock()?;
        state.require_open()?;
        state.root.verify_physical_root()?;
        if state.work >= 64 {
            return Err(Error::busy());
        }
        state.work += 1;
        Ok(Work {
            vault: self.clone(),
        })
    }
}
impl Work {
    pub fn archive_snapshot(
        &self,
        limits: crate::archive::ArchiveLimits,
    ) -> Result<crate::archive::ArchiveSnapshot> {
        let state = self.vault.lock()?;
        if state.phase == VaultPhase::Closed {
            return Err(Error::closed());
        }
        Ok(crate::archive::create_zip(&state.root, limits)?)
    }
    pub fn cancellation_requested(&self) -> bool {
        self.vault
            .lock()
            .map_or(true, |state| state.phase != VaultPhase::Open)
    }
    pub fn read_text(&self, path: &str) -> Result<TextSnapshot> {
        let state = self.vault.lock()?;
        if state.phase == VaultPhase::Closed {
            return Err(Error::closed());
        }
        operations::read(&state, path)
    }
    pub fn write_text(&self, path: &str, text: &str, expected: &ExpectedVersion) -> Result<String> {
        let state = self.vault.lock()?;
        if state.phase == VaultPhase::Closed {
            return Err(Error::closed());
        }
        operations::write(&state, path, text, expected, None)
    }
}
impl Drop for Work {
    fn drop(&mut self) {
        let mut state = self.vault.state.lock().unwrap_or_else(|e| e.into_inner());
        state.work -= 1;
        state.advance_close();
        self.vault.changed.notify_all();
    }
}
