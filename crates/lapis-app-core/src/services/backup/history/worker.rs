use super::HistoryIndexStatus;
use crate::services::VaultService;
use jj_vault::search::indexed;
use std::{
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub(super) fn run(
    vault: Weak<VaultService>,
    backup: Arc<Mutex<Option<tokio::sync::watch::Receiver<jj_vault::Status>>>>,
    stop: mpsc::Receiver<()>,
    progress: Arc<Mutex<HistoryIndexStatus>>,
    requested: Arc<AtomicBool>,
) {
    let mut due = Instant::now();
    loop {
        match stop.try_recv() {
            Ok(()) | Err(mpsc::TryRecvError::Disconnected) => return,
            Err(mpsc::TryRecvError::Empty) => {}
        }
        let changed = match backup.lock() {
            Ok(mut receiver) => match receiver.as_mut() {
                Some(backup) => match backup.has_changed() {
                    Ok(true) => {
                        backup.borrow_and_update();
                        true
                    }
                    Ok(false) => false,
                    Err(_) => {
                        // Tokio reports closed even with an unseen final value.
                        // Reconcile once, then retire the closed notification source.
                        *receiver = None;
                        true
                    }
                },
                None => false,
            },
            Err(_) => return,
        };
        if changed || requested.swap(false, Ordering::AcqRel) {
            due = Instant::now();
        }
        if Instant::now() >= due {
            let result = step(&vault);
            let Ok(mut state) = progress.lock() else {
                return;
            };
            state.passes = state.passes.saturating_add(1);
            match result {
                Ok(status) => {
                    state.source = Some(status);
                    state.error = None;
                }
                Err(error) => state.error = Some(error),
            }
            let delay = if state.error.is_some() {
                Duration::from_secs(5)
            } else if state.source.as_ref().is_some_and(|status| status.complete) {
                Duration::from_secs(60)
            } else {
                Duration::from_millis(30)
            };
            due = Instant::now() + delay;
        }
        // Bounded polling observes backup publication without an executor,
        // second worker, writer lease or dependence on renderer receivers.
        let delay = due
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(250));
        match stop.recv_timeout(delay) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}
fn step(vault: &Weak<VaultService>) -> Result<indexed::Status, String> {
    let vault = vault.upgrade().ok_or("Vault service is closed")?;
    let _work = vault.begin_work().map_err(|error| error.to_string())?;
    let _access = vault
        .history_index_access
        .lock()
        .map_err(|_| "History index access is unavailable")?;
    let source = vault
        .backup_storage
        .as_ref()
        .ok_or("History storage is not selected")?
        .history_source(&vault)
        .map_err(|error| error.to_string())?;
    // Resolve current installed layout for every immutable batch. Neither
    // admission nor cache repair acquires the capture/file-writer lease.
    #[cfg(test)]
    if let Some(pause) = vault.history_index_pause.lock().unwrap().take() {
        pause();
    }
    let status = source.index_step()?;
    let current = vault
        .backup_storage
        .as_ref()
        .unwrap()
        .history_source(&vault)
        .map_err(|error| error.to_string())?;
    if current.binding.state != source.binding.state {
        return Err("History storage changed during indexing".into());
    }
    Ok(status)
}
