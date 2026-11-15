//! Accepted metadata import uses the actual capture worker when it is running.
use super::*;
use crate::services::VaultService;
use jj_store_jj::vault_workspace::Session;
use jj_vault::FetchedHead;
use std::sync::Arc;
pub(in crate::services) async fn import(
    vault: &Arc<VaultService>,
    head: FetchedHead,
) -> Result<()> {
    let owner = vault.backup.lock().map_err(|_| failure())?.clone();
    if let Some(owner) = owner {
        owner
            .engine
            .lock()
            .await
            .import_remote(head)
            .await
            .map_err(|_| failure())?;
    } else {
        // The caller holds accepted-work and the native backup transition turn.
        // No disabled capture worker is started and no files are snapshotted.
        let storage = vault.backup_storage.as_ref().ok_or_else(failure)?;
        let binding = storage.history_source(vault)?.binding;
        Session::open(binding)
            .await
            .map_err(|_| failure())?
            .import_fetched(&head)
            .await
            .map_err(|_| failure())?;
    }
    Ok(())
}
