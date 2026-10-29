//! Docs, Search, and Tasks indexing that call sibling projection crates stay in
//! `indexing/`. This module is the workspace build, which has no path to those crates.

#[path = "indexing/manifest.rs"]
pub(in crate::services) mod manifest;
#[cfg(test)]
#[path = "indexing/status_tests.rs"]
mod status_tests;

use super::{Client, Error, Result, VaultService};
use lapis_app_protocol::{
    ErrorCode,
    database::search::source::SearchChunkingSettings,
    indexing::{IndexingCommand, IndexingValue},
};

pub(in crate::services) mod records {
    pub(in crate::services) fn markdown(path: &str) -> bool {
        path.rsplit_once('.').is_some_and(|(_, extension)| {
            extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown")
        })
    }
}

pub(in crate::services) struct Owner;

/// Explicit native source options. Host configuration and custom providers remain separate.
#[derive(Clone, Debug)]
pub struct SearchIndexingConfiguration {
    pub canvas: bool,
    pub chunking: SearchChunkingSettings,
}
impl Default for SearchIndexingConfiguration {
    fn default() -> Self {
        Self {
            canvas: false,
            chunking: SearchChunkingSettings {
                target_chars: 1200.0,
                breakpoint_window_chars: 320.0,
                breakpoint_decay: 0.7,
            },
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TrackedTaskStatus {
    pub date: String,
    pub attempted: usize,
    pub materialized: usize,
    pub changed: usize,
    pub failures: Vec<String>,
}

/// Native observations extend shared wire progress without duplicating its DTO.
#[derive(Clone, Debug, Default)]
pub struct IndexingStatus {
    pub progress: lapis_app_protocol::indexing::IndexingStatus,
    pub tracked: Option<TrackedTaskStatus>,
}
impl std::ops::Deref for IndexingStatus {
    type Target = lapis_app_protocol::indexing::IndexingStatus;
    fn deref(&self) -> &Self::Target {
        &self.progress
    }
}
impl std::ops::DerefMut for IndexingStatus {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.progress
    }
}

fn unavailable() -> Error {
    Error::new(
        ErrorCode::Unsupported,
        "Native projection crates are not workspace path members",
    )
}

impl VaultService {
    pub(super) fn finish_indexing(&self) -> Result<()> {
        let _owner = self.indexing.lock().map_err(|_| Error::closed())?.take();
        Ok(())
    }
    pub fn indexing_status(&self) -> Result<IndexingStatus> {
        Err(unavailable())
    }
}

impl Client {
    /// One admitted operation; sibling projection crates are not part of this workspace.
    pub async fn control_indexing(&self, _command: IndexingCommand) -> Result<IndexingValue> {
        self.vault.lock()?.require_client(self.id)?;
        Err(unavailable())
    }
}
