//! Native default-daily source admission; host owners select this journal separately.
mod diagnostic;
mod execute;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod guard_tests;
mod inspect;
#[cfg(test)]
mod process_tests;
mod record;
#[cfg(test)]
mod reference_tests;
#[cfg(test)]
mod tests;

use crate::{
    record_store::{RecordStore, StoredRecord},
    services::{
        Error, VaultService, Work,
        daily::{DailyDocumentOperationError, DailyDocumentSettings, DailyFormatContext},
    },
};
use lapis_app_protocol::{ErrorCode, ExpectedVersion, TextSnapshot, VaultPhase};
use lapis_task_services::TaskLifecycleAction;
use record::{Record, Stage};
pub use record::{TaskLifecycleFailure, TaskLifecyclePolicy, TaskLifecycleRequest};
use std::{
    fmt, io,
    path::Path,
    sync::{Arc, Weak},
};
use uuid::Uuid;

pub struct TaskLifecycleJournal {
    storage: RecordStore<Record>,
    vault: Weak<VaultService>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum TaskLifecycleDisposition {
    Retryable,
    Divergent,
    Completed,
}
#[derive(Clone, Debug)]
pub struct TaskLifecycleStatus {
    pub request: TaskLifecycleRequest,
    pub disposition: TaskLifecycleDisposition,
    pub original: TextSnapshot,
    pub task_replacement: String,
    pub daily: Option<TextSnapshot>,
    pub daily_replacement: Option<String>,
    pub last_failure: Option<TaskLifecycleFailure>,
}
#[derive(Debug)]
pub enum TaskRecoveryError {
    Service(Error),
    Task(super::super::TaskWriteError),
    Daily(DailyDocumentOperationError),
    Journal(io::Error),
    Partial {
        status: Box<TaskLifecycleStatus>,
        cause: Box<TaskRecoveryError>,
    },
}
impl fmt::Display for TaskRecoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Service(e) => e.fmt(f),
            Self::Task(e) => e.fmt(f),
            Self::Daily(e) => e.fmt(f),
            Self::Journal(e) => e.fmt(f),
            Self::Partial { cause, .. } => cause.fmt(f),
        }
    }
}
impl std::error::Error for TaskRecoveryError {}
impl From<Error> for TaskRecoveryError {
    fn from(e: Error) -> Self {
        Self::Service(e)
    }
}
impl From<super::super::TaskWriteError> for TaskRecoveryError {
    fn from(e: super::super::TaskWriteError) -> Self {
        Self::Task(e)
    }
}
impl From<DailyDocumentOperationError> for TaskRecoveryError {
    fn from(e: DailyDocumentOperationError) -> Self {
        Self::Daily(e)
    }
}
impl From<io::Error> for TaskRecoveryError {
    fn from(e: io::Error) -> Self {
        Self::Journal(e)
    }
}
type Result<T> = std::result::Result<T, TaskRecoveryError>;
#[derive(Clone, Copy, Debug, PartialEq)]
enum Checkpoint {
    Accepted,
    Ensuring,
    Ensured,
    Observing,
    Observed,
    Advancing,
    Advanced,
}
fn conflict() -> Error {
    Error::new(
        ErrorCode::Conflict,
        "Task lifecycle content diverged from its pinned intent",
    )
}
impl TaskLifecycleJournal {
    pub(in crate::services::tasks) fn retire(&mut self) -> io::Result<()> {
        self.storage.retire()
    }
    /// Native installation construction only; views cannot own this lifetime.
    pub fn open(directory: &Path, vault: &Arc<VaultService>) -> Result<Self> {
        let state = vault.lock()?;
        state.require_open()?;
        let storage = RecordStore::open(directory, vault.id, &state.root)?;
        for id in storage.ids()? {
            storage.read(id)?.ok_or_else(record::invalid)?;
        }
        Ok(Self {
            storage,
            vault: Arc::downgrade(vault),
        })
    }
    fn verify(&self, work: &Work) -> Result<()> {
        let vault = self.vault.upgrade().ok_or_else(Error::closed)?;
        if !Arc::ptr_eq(&vault, &work.vault) {
            return Err(Error::denied().into());
        }
        let state = vault.lock()?;
        if state.phase == VaultPhase::Closed {
            return Err(Error::closed().into());
        }
        state.root.verify_physical_root()?;
        self.storage.verify()?;
        Ok(())
    }
    pub fn submit(
        &mut self,
        work: Work,
        request: TaskLifecycleRequest,
        policy: &TaskLifecyclePolicy,
    ) -> Result<TaskLifecycleStatus> {
        self.submit_with(work, request, policy, |_| Ok(()))
    }
    fn submit_with(
        &mut self,
        work: Work,
        request: TaskLifecycleRequest,
        policy: &TaskLifecyclePolicy,
        mut checkpoint: impl FnMut(Checkpoint) -> io::Result<()>,
    ) -> Result<TaskLifecycleStatus> {
        let vault = work.vault.clone();
        let _gate = vault.task_lifecycle.lock().map_err(|_| Error::busy())?;
        self.verify(&work)?;
        if let Some((record, _)) = self.storage.read(request.request_id)? {
            if !record::equal(&record.request, &request) {
                return Err(Error::new(
                    ErrorCode::InvalidRequest,
                    "Task lifecycle request identity was reused with different input",
                )
                .into());
            }
            return self.status(&work, &record);
        }
        if self.storage.ids()?.len() >= Record::MAX_RECORDS {
            return Err(Error::busy().into());
        }
        let expected = ExpectedVersion::Version {
            version: request.expected_version.clone(),
        };
        let original = super::original(&work, &request.path, &expected)?;
        let plan = lapis_task_services::plan_task_lifecycle(
            &original.text,
            &request.path,
            &request.action,
            &policy.day_parts,
        )
        .map_err(super::super::TaskWriteError::from)?
        .into();
        let mut record = Record {
            schema_version: 1,
            vault_id: vault.id,
            request,
            policy: policy.clone(),
            original,
            plan,
            daily: None,
            stage: Stage::Accepted,
            last_failure: None,
        };
        let mut expected = self.storage.write(&record, None)?;
        checkpoint(Checkpoint::Accepted)?;
        if let Err(cause) = self.drive(&work, &mut record, &mut expected, &mut checkpoint) {
            return Err(self.failed(&work, &mut record, &mut expected, cause));
        }
        self.status(&work, &record)
    }
    /// Explicit retry observes and reuses the accepted plan; it never selects today anew.
    pub fn retry(&mut self, work: Work, id: Uuid) -> Result<TaskLifecycleStatus> {
        let vault = work.vault.clone();
        let _gate = vault.task_lifecycle.lock().map_err(|_| Error::busy())?;
        self.verify(&work)?;
        let (mut record, mut expected) = self.storage.read(id)?.ok_or_else(|| {
            Error::new(ErrorCode::NotFound, "Task lifecycle request is unavailable")
        })?;
        if self.status(&work, &record)?.disposition == TaskLifecycleDisposition::Divergent {
            return Err(conflict().into());
        }
        if record.last_failure.take().is_some() {
            self.publish(&record, &mut expected)?;
        }
        if let Err(cause) = self.drive(&work, &mut record, &mut expected, &mut |_| Ok(())) {
            return Err(self.failed(&work, &mut record, &mut expected, cause));
        }
        self.status(&work, &record)
    }
    fn failed(
        &self,
        work: &Work,
        record: &mut Record,
        expected: &mut Vec<u8>,
        cause: TaskRecoveryError,
    ) -> TaskRecoveryError {
        if let Some(failure) = diagnostic::known(&cause) {
            record.last_failure = Some(failure);
            if let Err(error) = self.publish(record, expected) {
                return error;
            }
        }
        match self.status(work, record) {
            Ok(status) => TaskRecoveryError::Partial {
                status: Box::new(status),
                cause: Box::new(cause),
            },
            Err(_) => cause,
        }
    }
    fn publish(&self, record: &Record, expected: &mut Vec<u8>) -> Result<()> {
        *expected = self.storage.write(record, Some(expected))?;
        Ok(())
    }
}
