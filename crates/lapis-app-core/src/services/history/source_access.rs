//! Physical source witnesses for accepted native work, independent of reader handles.
use super::{policy::Policy, source::Source};
use crate::services::VaultService;
use jj_history_service::{Error, Result, local};
use jj_store_model::RepositoryId;
use lapis_app_protocol::{VaultPhase, repository_remotes::RemoteRepositoryTarget as Target};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Weak},
};

pub(in crate::services) struct SourceAccess {
    vault: Weak<VaultService>,
    source: Source,
    target: Target,
    root: PathBuf,
    identity: (RepositoryId, u64),
}
impl SourceAccess {
    /// Only an already accepted native operation may retain this witness.
    /// It carries no mutation authority or permanent Work lease.
    pub fn read(vault: &Arc<VaultService>, target: Target) -> Result<Arc<Self>> {
        let source = selected(vault, target)?.ok_or(Error::Unavailable)?;
        let state = vault.lock().map_err(|_| Error::Denied)?;
        state
            .root
            .verify_physical_root()
            .map_err(|_| Error::Denied)?;
        let root = state.root.path().to_path_buf();
        drop(state);
        Ok(Arc::new(Self {
            vault: Arc::downgrade(vault),
            identity: Policy::identity(vault, (target == Target::Original).then_some(&source)),
            source,
            target,
            root,
        }))
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
}
fn selected(vault: &VaultService, target: Target) -> Result<Option<Source>> {
    match target {
        Target::Backup => Policy::source(vault),
        Target::Original => Policy::original(vault),
    }
}
#[async_trait::async_trait]
impl local::Access for SourceAccess {
    fn repository(&self) -> RepositoryId {
        self.identity.0
    }
    fn generation(&self) -> u64 {
        self.identity.1
    }
    fn metadata_path(&self) -> &Path {
        &self.source.metadata
    }
    async fn authorize(&self) -> Result<()> {
        let vault = self.vault.upgrade().ok_or(Error::Denied)?;
        let state = vault.lock().map_err(|_| Error::Denied)?;
        if state.phase == VaultPhase::Closed || state.root.path() != self.root {
            return Err(Error::Denied);
        }
        state
            .root
            .verify_physical_root()
            .map_err(|_| Error::Denied)?;
        drop(state);
        self.source.verify().map_err(|_| Error::EpochChanged)?;
        if selected(&vault, self.target)?.as_ref() != Some(&self.source) {
            return Err(Error::EpochChanged);
        }
        Ok(())
    }
}
