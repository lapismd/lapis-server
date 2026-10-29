//! Vault-owned derived work remains independent of backup capture and clients.
mod controls;
mod requests;
mod selection;
#[cfg(test)]
mod tests;
mod worker;

use crate::services::{Error, Result, VaultService};
use jj_vault::search::indexed;
use lapis_app_protocol::ErrorCode;
use std::{
    sync::{Arc, Mutex, Weak, atomic::AtomicBool, mpsc},
    thread,
};

#[derive(Clone, Debug, Default)]
pub struct HistoryIndexStatus {
    pub passes: u64,
    pub source: Option<indexed::Status>,
    pub error: Option<String>,
}
pub(in crate::services) struct Owner {
    pub status: Arc<Mutex<HistoryIndexStatus>>,
    stop: mpsc::SyncSender<()>,
    backup: Arc<Mutex<Option<tokio::sync::watch::Receiver<jj_vault::Status>>>>,
    requested: Arc<AtomicBool>,
    thread: Mutex<Option<thread::JoinHandle<()>>>,
}
impl Owner {
    pub fn start(
        vault: Weak<VaultService>,
        backup: Option<tokio::sync::watch::Receiver<jj_vault::Status>>,
    ) -> Result<Self> {
        let (stop, receiver) = mpsc::sync_channel(1);
        let status = Arc::new(Mutex::new(HistoryIndexStatus::default()));
        let progress = status.clone();
        let backup = Arc::new(Mutex::new(backup));
        let notifications = backup.clone();
        let requested = Arc::new(AtomicBool::new(false));
        let wake = requested.clone();
        let thread = thread::Builder::new()
            .name("lapis-history-index".into())
            .spawn(move || worker::run(vault, notifications, receiver, progress, wake))
            .map_err(|_| unavailable())?;
        Ok(Self {
            status,
            stop,
            backup,
            requested,
            thread: Mutex::new(Some(thread)),
        })
    }
    pub fn finish(&self) -> Result<()> {
        let _ = self.stop.try_send(());
        let mut slot = self.thread.lock().map_err(|_| unavailable())?;
        if let Some(worker) = slot.take() {
            // Releasing the last upgraded vault Arc can drop this owner on
            // its own worker. Stop is signalled; the current batch will return.
            if worker.thread().id() == thread::current().id() {
                return Ok(());
            }
            if worker.join().is_err() {
                self.status.lock().map_err(|_| unavailable())?.error =
                    Some("Native History index worker stopped".into());
                return Err(unavailable());
            }
        }
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn is_finished(&self) -> bool {
        self.thread.lock().unwrap().is_none()
    }
}
fn not_selected() -> Error {
    Error::new(
        ErrorCode::Unsupported,
        "Native History indexing is not selected",
    )
}
impl Drop for Owner {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}
fn unavailable() -> Error {
    Error::new(ErrorCode::Io, "Native History index owner is unavailable")
}
