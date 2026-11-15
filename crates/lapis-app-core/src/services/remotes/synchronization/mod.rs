//! A vault-owned scheduler; idle threads retain neither the vault nor a work lease.
mod cycle;
mod journal;
mod observation;
mod recovery;
mod worker;
use super::*;
use lapis_app_protocol::repository_sync::*;
use std::sync::{
    Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub(in crate::services) struct Owner {
    control: Arc<Control>,
    started: AtomicBool,
}
struct Control {
    status: Mutex<RepositorySyncSnapshot>,
    requested: Mutex<u64>,
    changed: Condvar,
    stopped: AtomicBool,
}
impl Default for Owner {
    fn default() -> Self {
        Self {
            control: Arc::new(Control {
                status: Mutex::new(RepositorySyncSnapshot {
                    revision: 0,
                    enabled: false,
                    busy: false,
                    phase: RepositorySyncPhase::Paused,
                    proposal: None,
                    conflict: None,
                    last_success: None,
                    recovery: false,
                    pending_apply: None,
                }),
                requested: Mutex::new(0),
                changed: Condvar::new(),
                stopped: AtomicBool::new(false),
            }),
            started: AtomicBool::new(false),
        }
    }
}
impl Owner {
    pub(in crate::services) fn start(&self, vault: &Arc<VaultService>) -> Result<()> {
        if !vault.remotes_storage.as_ref().is_some_and(|s| s.transfers)
            || vault.backup_storage.is_none()
            || self.started.swap(true, Ordering::AcqRel)
        {
            return Ok(());
        }
        let control = self.control.clone();
        let vault = Arc::downgrade(vault);
        std::thread::Builder::new()
            .name("lapis-backup-sync".into())
            .spawn(move || worker::run(vault, control))?;
        Ok(())
    }
    pub(in crate::services) fn selection(&self, state: &SelectionState) {
        if let Ok(mut status) = self.control.status.lock() {
            status.revision = state.revision;
            status.enabled = state.automatic_sync;
            if !status.busy && !status.recovery && status.proposal.is_none() {
                status.phase = if status.enabled {
                    RepositorySyncPhase::Ready
                } else {
                    RepositorySyncPhase::Paused
                };
            }
        }
    }
    pub(in crate::services) fn wake(&self) {
        self.control.wake();
    }
    pub(in crate::services) fn stop(&self) {
        self.control.stopped.store(true, Ordering::Release);
        self.control.wake();
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.stop();
    }
}
impl Control {
    fn wake(&self) {
        if let Ok(mut requested) = self.requested.lock() {
            *requested = requested.wrapping_add(1);
        }
        self.changed.notify_all();
    }
    fn update(&self, change: impl FnOnce(&mut RepositorySyncSnapshot)) -> Result<()> {
        let mut status = self.status.lock().map_err(|_| Error::busy())?;
        change(&mut status);
        Ok(())
    }
}
impl Client {
    /// Passive observation never queues behind network work or owns its lifetime.
    pub fn repository_synchronization(&self) -> Result<RepositorySyncSnapshot> {
        self.vault.lock()?.require_client(self.id)?;
        self.vault.lock()?.root.verify_physical_root()?;
        Ok(self
            .vault
            .synchronization
            .control
            .status
            .lock()
            .map_err(|_| Error::busy())?
            .clone())
    }
    pub async fn control_repository_synchronization(
        &self,
        command: RepositorySyncCommand,
    ) -> Result<RepositorySyncSnapshot> {
        self.vault.lock()?.require_client(self.id)?;
        let work = self.vault.begin_work()?;
        let turn = self.vault.backup.reserve()?;
        let vault = self.vault.clone();
        turn.run(work, move || cycle::execute(&vault, Some(command)))
            .await?;
        self.repository_synchronization()
    }
}
pub(super) fn pending(selection: &Selection) -> Result<bool> {
    journal::pending(selection)
}
