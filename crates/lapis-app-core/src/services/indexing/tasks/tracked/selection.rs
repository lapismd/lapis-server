use crate::services::{
    Error, Result, TaskLifecyclePolicy, VaultService,
    daily::{DailyDocumentError, DefaultTaskObservationError},
};
use lapis_app_protocol::TextSnapshot;
use std::{fmt, sync::Arc};

/// Native application selection only. An unavailable provider never falls back.
#[derive(Clone, Debug)]
pub enum TrackedTaskProvider {
    Default(TaskLifecyclePolicy),
    Unavailable(DailyDocumentError),
}
#[derive(Clone, Debug, Default)]
pub struct TrackedTaskStatus {
    pub date: String,
    pub attempted: usize,
    pub materialized: usize,
    pub changed: usize,
    pub failures: Vec<TrackedTaskFailure>,
}
#[derive(Clone, Debug)]
pub struct TrackedTaskFailure {
    pub path: String,
    pub cause: Arc<TrackedTaskError>,
    /// Readable current daily content, including a seed retained after failure.
    pub retained_daily: Option<TextSnapshot>,
}
#[derive(Debug)]
pub enum TrackedTaskError {
    Source(Error),
    Provider(DailyDocumentError),
    Daily(DefaultTaskObservationError),
}
impl fmt::Display for TrackedTaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(e) => e.fmt(f),
            Self::Provider(e) => e.fmt(f),
            Self::Daily(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for TrackedTaskError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Source(e) => e,
            Self::Provider(e) => e,
            Self::Daily(e) => e,
        })
    }
}
impl From<Error> for TrackedTaskError {
    fn from(error: Error) -> Self {
        Self::Source(error)
    }
}

impl VaultService {
    /// The native installation supplies default-provider availability explicitly.
    /// This never selects a host, changes authored Tasks or creates another owner.
    pub async fn set_tracked_task_materialization(
        self: &Arc<Self>,
        provider: Option<TrackedTaskProvider>,
    ) -> Result<()> {
        if provider.is_some() {
            self.start_indexing(true, true, super::super::super::owner::Clock::default())
                .await?;
        }
        let _work = self.begin_work()?;
        if let Some(owner) = self
            .indexing
            .lock()
            .map_err(|_| super::super::super::unavailable())?
            .as_ref()
        {
            owner.select_tracking(provider)?;
        }
        Ok(())
    }
}
