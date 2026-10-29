use super::HistoryIndexStatus;
use crate::services::VaultService;
use jj_vault::search::indexed;
use sha2::{Digest, Sha256};
use std::{
    fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex, Weak,
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
            let busy = matches!(result, Err(ref error) if error == "History index is busy");
            match result {
                Ok(status) => {
                    state.source = Some(status);
                    state.error = None;
                }
                Err(_) if busy => state.error = None,
                Err(error) => state.error = Some(error),
            }
            let delay = if busy {
                Duration::from_millis(30)
            } else if state.error.is_some() {
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
    // Publish a finished index directory before readers can see the cache root.
    // The indexer creates that root before its index, and a reader that wins
    // the owner lock then observes a missing index and blocks publication.
    publish_index(&source)?;
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

fn publish_index(history: &jj_vault::search::NativeHistory) -> Result<(), String> {
    // A read may index an existing repository. It must not create one.
    if !history.binding.state.is_dir() {
        return Err("History repository is not initialized".into());
    }
    let namespace = serde_json::to_string(&(&history.binding.vault_id, &history.exclusions))
        .map_err(|error| error.to_string())?;
    let key = lower_hex(Sha256::digest(namespace.as_bytes()));
    let parent = history.binding.state.join("history-search-v2");
    let root = parent.join(&key);
    if root.join("index").join("meta.json").is_file() {
        return Ok(());
    }
    let staging = history
        .binding
        .state
        .join(".history-search-staging")
        .join(&key);
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    drop(indexed::Engine::open(&staging, &namespace)?);
    fs::create_dir_all(&parent).map_err(|error| error.to_string())?;
    match fs::rename(&staging, &root) {
        Ok(()) => Ok(()),
        Err(error) if root.join("index").join("meta.json").is_file() => {
            let _ = fs::remove_dir_all(&staging);
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    }
}

fn lower_hex(bytes: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.as_ref().len() * 2);
    for byte in bytes.as_ref() {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}
