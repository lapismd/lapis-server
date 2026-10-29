//! The vault owns the existing native engine; clients own passive status views.
mod close;
mod commands;
mod configuration;
mod control;
mod diagnostics;
mod dto;
pub(super) mod history;
#[cfg(test)]
mod history_tests;
mod provider;
mod restore;
mod slot;
pub(in crate::services) use slot::Slot;
mod storage;
#[cfg(test)]
mod tests;
mod watch;

use super::{Client, Error, Result};
pub use history::HistoryIndexStatus;
use jj_vault::{Backup, Status};
use std::sync::{Arc, Mutex};
pub use storage::BackupStorage;
use tokio::sync::{Mutex as AsyncMutex, watch::Receiver};

pub(super) struct Owner {
    engine: AsyncMutex<Backup>,
    status: Receiver<Status>,
    watch: Mutex<Option<notify::RecommendedWatcher>>,
}
fn failure() -> Error {
    Error::new(
        lapis_app_protocol::ErrorCode::Io,
        "Automatic vault backup is unavailable",
    )
}
impl Client {
    /// Reconciliation is native accepted work; cancellation of this waiter
    /// cannot abandon initialization. Receivers own neither engine nor watch.
    pub async fn start_backup(&self) -> Result<Receiver<Status>> {
        self.vault.lock()?.require_client(self.id)?;
        let vault = self.vault.clone();
        let work = vault.begin_work()?;
        let ticket = vault.backup.reserve()?;
        let owner = ticket
            .run(work, move || {
                let storage = vault.backup_storage.as_ref().ok_or_else(|| {
                    Error::new(
                        lapis_app_protocol::ErrorCode::Unsupported,
                        "Automatic backup is not selected",
                    )
                })?;
                storage.validate_vault(&vault.lock()?.root)?;
                let mut slot = vault.backup.lock().map_err(|_| failure())?;
                if !configuration::read(&vault)?.enabled {
                    commands::stop(&mut slot)?;
                    return Ok(None);
                }
                if let Some(owner) = slot.as_ref() {
                    return Ok(Some(owner.clone()));
                }
                let owner = Arc::new(storage.start(&vault)?);
                *slot = Some(owner.clone());
                Ok::<_, Error>(Some(owner))
            })
            .await?;
        Ok(match owner {
            Some(owner) => owner.status.clone(),
            None => {
                tokio::sync::watch::channel(Status {
                    phase: jj_vault::Phase::Closed,
                    checkpoint: None,
                    error: None,
                })
                .1
            }
        })
    }
}
