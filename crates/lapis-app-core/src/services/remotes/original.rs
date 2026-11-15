//! Passive discovery and explicit config save retain distinct native capabilities.
use super::*;
use crate::{
    repository::remotes::selection::OriginalObservation,
    services::history::source_access::SourceAccess,
};
use jj_history_service::local::{self, Access as _};
use jj_store_jj::original_remotes::{MutationAccess, OriginalRepository, Snapshot};
use std::{io, path::Path};

pub(super) struct Discovery {
    pub snapshot: Snapshot,
    pub observed: OriginalObservation,
    pub repository: OriginalRepository,
}
pub(super) async fn read(vault: &Arc<VaultService>) -> Result<Option<Discovery>> {
    let access = match SourceAccess::read(vault, RemoteRepositoryTarget::Original) {
        Ok(access) => access,
        Err(jj_history_service::Error::Unavailable) => return Ok(None),
        Err(_) => return Err(unavailable()),
    };
    let source = access.fingerprint();
    let root = access.root().to_path_buf();
    let repository = OriginalRepository::open_readonly(access, root)
        .await
        .map_err(|_| unavailable())?;
    let snapshot = repository.snapshot().await.map_err(|_| unavailable())?;
    Ok(Some(Discovery {
        observed: OriginalObservation {
            source,
            configuration: snapshot.remote_configuration.clone(),
        },
        snapshot,
        repository,
    }))
}
fn unavailable() -> Error {
    Error::new(
        ErrorCode::Unsupported,
        "Original remote discovery is unavailable",
    )
}
/// Only the explicit accepted save path constructs this capability. It never
/// derives authority from a browser History handle or the passive reader owner.
pub(super) struct Writer(pub Arc<SourceAccess>);
#[async_trait::async_trait]
impl local::Access for Writer {
    fn repository(&self) -> jj_store_model::RepositoryId {
        self.0.repository()
    }
    fn generation(&self) -> u64 {
        self.0.generation()
    }
    fn metadata_path(&self) -> &Path {
        self.0.metadata_path()
    }
    async fn authorize(&self) -> local::Result<()> {
        self.0.authorize().await
    }
}
#[async_trait::async_trait]
impl MutationAccess for Writer {
    async fn authorize_mutation(&self) -> local::Result<()> {
        self.0.authorize().await
    }
}
pub(super) fn save(
    vault: &Arc<VaultService>,
    selection: &Selection,
    expected: u64,
    connection: RemoteConnection,
    discovery: Option<&Discovery>,
    runtime: &tokio::runtime::Runtime,
) -> Result<()> {
    let discovery = discovery.ok_or_else(unavailable)?;
    if !discovery
        .snapshot
        .bookmarks
        .iter()
        .any(|b| b.name == connection.bookmark && b.commit.is_some() && !b.conflicted)
    {
        return Err(Error::new(
            ErrorCode::InvalidRequest,
            "Select an existing resolved local bookmark",
        ));
    }
    let access =
        SourceAccess::read(vault, RemoteRepositoryTarget::Original).map_err(|_| unavailable())?;
    if access.fingerprint() != discovery.observed.source {
        return Err(unavailable());
    }
    let root = access.root().to_path_buf();
    let repository = runtime
        .block_on(OriginalRepository::open(Arc::new(Writer(access)), root))
        .map_err(|_| unavailable())?;
    let binding = repository
        .transport_binding(selection.private_path())
        .map_err(|_| unavailable())?;
    let target = jj_vault::RemoteTarget {
        url: connection.url.clone(),
        branch: connection.branch.clone(),
    };
    let observed = discovery.observed.clone();
    selection.configure_original(
        expected,
        connection.clone(),
        observed.clone(),
        || {
            binding
                .pending_publication()
                .map(|p| p.is_some())
                .map_err(|_| io::Error::other("Earlier original publication needs review"))
        },
        || {
            let saved = runtime
                .block_on(repository.save_remote(
                    &observed.configuration,
                    &connection.name,
                    &target,
                ))
                .map_err(|error| match error {
                    jj_store_jj::original_remotes::Error::Stale => {
                        io::Error::other(crate::repository::remotes::selection::SelectionConflict)
                    }
                    _ => io::Error::other("Original configuration changed or could not be saved"),
                })?;
            Ok(OriginalObservation {
                source: observed.source.clone(),
                configuration: saved.remote_configuration,
            })
        },
    )?;
    Ok(())
}
