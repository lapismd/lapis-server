//! Accepted native remote work survives waiter cancellation; hosts retain custody.
mod backup;
mod backup_journal;
mod backup_operations;
mod custody;
mod journal;
mod operations;
mod original;
mod review_storage;
mod snapshot;
mod storage;
mod transfer;
use super::{Client, Error, Result, VaultService};
use crate::repository::remotes::selection::{Selection, SelectionState};
use lapis_app_protocol::{ErrorCode, repository_remotes::*};
use std::sync::Arc;
pub use storage::{
    RemoteCredentialAction, RemoteCredentialFuture, RemoteCredentials, RemoteStorage,
};

fn unsupported() -> Error {
    Error::new(
        ErrorCode::Unsupported,
        "This repository remote operation is unavailable",
    )
}
impl Client {
    pub async fn repository_remote_settings(&self) -> Result<RepositoryRemotesSnapshot> {
        self.remote_work(None).await
    }
    pub async fn control_repository_remotes(
        &self,
        command: RepositoryRemotesCommand,
    ) -> Result<RepositoryRemotesSnapshot> {
        self.remote_work(Some(command)).await
    }
    async fn remote_work(
        &self,
        command: Option<RepositoryRemotesCommand>,
    ) -> Result<RepositoryRemotesSnapshot> {
        self.vault.lock()?.require_client(self.id)?;
        let work = self.vault.begin_work()?;
        let turn = self.vault.backup.reserve()?;
        let vault = self.vault.clone();
        let result = turn
            .run(work, move || {
                let storage = vault.remotes_storage.as_ref().ok_or_else(unsupported)?;
                let _gate = storage.gate.lock().map_err(|_| Error::busy())?;
                let _process = custody::lock(storage)?;
                let selection = storage.selection(&vault)?;
                selection.observe_credentials(custody::epoch(storage)?)?;
                let executor = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                let discovered = executor.block_on(original::read(&vault))?;
                selection.observe_original(discovered.as_ref().map(|d| d.observed.clone()))?;
                if let Some(command) = command {
                    match command {
                        RepositoryRemotesCommand::SaveConnection {
                            expected_revision,
                            target,
                            connection,
                        } => {
                            // Credential scope admission precedes either private or Git config writes.
                            admit_connection(storage, &connection)?;
                            match target {
                                RemoteRepositoryTarget::Backup => {
                                    selection.configure(
                                        expected_revision,
                                        target,
                                        connection,
                                        || pending_backup(&vault).map_err(std::io::Error::other),
                                    )?;
                                }
                                RemoteRepositoryTarget::Original => {
                                    original::save(
                                        &vault,
                                        &selection,
                                        expected_revision,
                                        connection,
                                        discovered.as_ref(),
                                        &executor,
                                    )?;
                                }
                            }
                        }
                        command if custody::is_command(&command) => {
                            custody::run(storage, &selection, command, &executor)?;
                        }
                        command => {
                            operations::run(&vault, storage, &selection, command, &executor)?
                        }
                    }
                }
                let original = executor.block_on(original::read(&vault))?;
                let state =
                    selection.observe_original(original.as_ref().map(|d| d.observed.clone()))?;
                snapshot::read(storage, &selection, state, original, &vault, &executor)
            })
            .await;
        // Detach fences delivery, while the accepted worker keeps its own lease.
        self.vault.lock()?.require_client(self.id)?;
        result
    }
}
fn pending_backup(vault: &VaultService) -> Result<bool> {
    let Some(storage) = vault.backup_storage.as_ref() else {
        return Ok(false);
    };
    let source = storage.history_source(vault)?;
    source
        .binding
        .pending_publication()
        .map(|v| v.is_some())
        .map_err(|_| Error::new(ErrorCode::Io, "Earlier repository publication needs review"))
}

fn admit_connection(storage: &RemoteStorage, connection: &RemoteConnection) -> Result<()> {
    use RemoteAuthentication as Auth;
    if connection.authentication == Auth::Anonymous {
        return Ok(());
    }
    let inventory = storage.credentials.inventory()?;
    if matches!(connection.authentication, Auth::SshAgent | Auth::SshDefault) {
        return if inventory.ssh_available {
            Ok(())
        } else {
            Err(unsupported())
        };
    }
    if !matches!(inventory.storage, RemoteStorageState::Available) {
        return Err(Error::new(
            ErrorCode::PermissionDenied,
            "Credential storage is locked or unavailable",
        ));
    }
    let url = reqwest::Url::parse(&connection.url).map_err(|_| unsupported())?;
    let host = match (url.host_str(), url.port()) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_owned(),
        _ => return Err(unsupported()),
    };
    if !inventory.credentials.iter().any(|c| {
        c.id == connection.credential
            && c.available
            && c.host.eq_ignore_ascii_case(&host)
            && matches!(
                (connection.authentication, c.method),
                (Auth::Token, RemoteCredentialMethod::Token)
                    | (Auth::Forge, RemoteCredentialMethod::Forge)
            )
    }) {
        return Err(Error::new(
            ErrorCode::PermissionDenied,
            "Choose an available credential for this remote host",
        ));
    }
    Ok(())
}
