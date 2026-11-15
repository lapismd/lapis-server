//! Existing managed binding and private projection; reads never start capture.
use super::*;
use jj_store_jj::vault_workspace::HistoryReader;
use jj_vault::Binding;
use sha2::{Digest as _, Sha256};

pub(super) fn binding(vault: &VaultService) -> Result<Binding> {
    vault
        .backup_storage
        .as_ref()
        .ok_or_else(unsupported)?
        .history_source(vault)
        .map(|source| source.binding)
}
pub(super) fn source(vault: &VaultService) -> Result<String> {
    let state = vault.lock()?;
    state.root.verify_physical_root()?;
    let (device, inode) = state.root.physical_identity()?;
    let mut hash = Sha256::new_with_prefix(b"lapis/backup-remote-source/v1\0");
    hash.update(vault.id.as_bytes());
    hash.update(device.to_be_bytes());
    hash.update(inode.to_be_bytes());
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
pub(super) fn project(
    vault: &VaultService,
    storage: &RemoteStorage,
    selection: &Selection,
    state: &SelectionState,
    settings: &RemoteSettings,
    runtime: &tokio::runtime::Runtime,
) -> Result<RemoteRepositorySettings> {
    let status = snapshot::connection_status(state.backup.as_ref(), settings);
    let mut result = RemoteRepositorySettings {
        target: RemoteRepositoryTarget::Backup,
        connection: state.backup.clone(),
        status,
        bookmarks: vec![],
        remotes: None,
        can_fetch: false,
        can_push: false,
        observation: None,
        review: None,
        publication: None,
    };
    let binding = match binding(vault) {
        Err(error) if error.code() == ErrorCode::Unsupported => return Ok(result),
        value => value?,
    };
    let pending = operations::pending(&binding)?;
    if state.backup.is_some() {
        let saved = backup_journal::Journal::open(selection, state, &source(vault)?)?;
        result.observation = saved.observation;
        result.review = saved
            .review
            .as_ref()
            .filter(|r| !r.started && pending.is_none())
            .map(backup_journal::public)
            .transpose()?;
    }
    result.publication = pending.as_ref().map(|p| RemotePublication {
        id: p.request_id.clone(),
        commit: p.after.clone(),
        outcome: if p.outcome == jj_vault::PushOutcome::Stopped {
            RemotePublicationState::Stopped
        } else {
            RemotePublicationState::Unknown
        },
    });
    let admitted = storage.transfers
        && state.backup.is_some()
        && pending.is_none()
        && matches!(
            status,
            RemoteConnectionState::Connected | RemoteConnectionState::ReadOnly
        );
    result.can_fetch = admitted;
    if admitted
        && matches!(status, RemoteConnectionState::Connected)
        && binding.state.join("vault.json").try_exists()?
    {
        let has_checkpoint = runtime
            .block_on(async {
                HistoryReader::open(binding)
                    .await?
                    .history_revision(None, 0)
                    .await
            })
            .map_err(|_| transfer::failed())?
            .revision
            .is_some();
        result.can_push = has_checkpoint;
        if has_checkpoint {
            result.bookmarks.push("lapis-backup".into());
        }
    }
    Ok(result)
}
