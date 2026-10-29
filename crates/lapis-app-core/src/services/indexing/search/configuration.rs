use crate::{database::Database, services::Result};
use lapis_app_protocol::database::search::source::SearchChunkingSettings;
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};

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
impl SearchIndexingConfiguration {
    pub(super) fn validate(&self) -> Result<()> {
        Database::validate_search_chunking(&self.chunking)
    }
}
#[derive(Default)]
pub(in crate::services::indexing) struct Selection {
    generation: AtomicU64,
    value: Mutex<Option<Policy>>,
}
#[derive(Clone)]
pub(in crate::services::indexing) struct Policy {
    pub options: SearchIndexingConfiguration,
    pub markdown: bool,
    pub prune_unselected: bool,
}
impl From<SearchIndexingConfiguration> for Policy {
    fn from(options: SearchIndexingConfiguration) -> Self {
        Self {
            options,
            markdown: true,
            prune_unselected: false,
        }
    }
}
impl Selection {
    pub(super) fn snapshot(&self) -> Result<Option<(Policy, u64)>> {
        let value = self.value.lock().map_err(|_| super::super::unavailable())?;
        Ok(value
            .clone()
            .map(|value| (value, self.generation.load(Ordering::Acquire))))
    }
    pub(in crate::services::indexing) fn configure(&self, value: Option<Policy>) -> Result<()> {
        let mut current = self.value.lock().map_err(|_| super::super::unavailable())?;
        *current = value;
        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    pub(super) fn current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::Acquire) == generation
    }
}
