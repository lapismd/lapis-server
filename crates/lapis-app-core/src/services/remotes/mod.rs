//! Accepted native remote work survives waiter cancellation; hosts retain custody.
mod snapshot;
mod storage;
use super::{Client, Error, Result, VaultService};
use crate::repository::remotes::selection::{Selection, SelectionState};
use lapis_app_protocol::{ErrorCode, repository_remotes::*};
use std::sync::Arc;
pub use storage::{RemoteCredentials, RemoteStorage};

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
                let selection = storage.selection(&vault)?;
                if let Some(command) = command {
                    match command {
                        RepositoryRemotesCommand::SaveConnection {
                            expected_revision,
                            target: RemoteRepositoryTarget::Backup,
                            connection,
                        } => {
                            selection.configure(
                                expected_revision,
                                RemoteRepositoryTarget::Backup,
                                connection,
                                || pending_backup(&vault).map_err(std::io::Error::other),
                            )?;
                        }
                        _ => return Err(unsupported()),
                    }
                }
                snapshot::read(storage, selection.read()?)
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
    if !source.binding.state.join("vault.json").try_exists()? {
        return Ok(false);
    }
    source
        .binding
        .pending_publication()
        .map(|v| v.is_some())
        .map_err(|_| Error::new(ErrorCode::Io, "Earlier repository publication needs review"))
}
