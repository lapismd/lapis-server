//! Task transforms that call sibling projection crates stay in `tasks/`.
//! This module is the workspace build, which has no path to those crates.

#[path = "tasks/service/storage.rs"]
mod storage;

pub use storage::TaskStorage;

pub(in crate::services) mod service {
    pub(in crate::services) struct Owner;
}

use super::{Client, Error, Result, VaultService};
use lapis_app_protocol::{
    ErrorCode,
    native_tasks::{NativeTaskCommand, NativeTaskValue},
};
use std::fmt;

#[derive(Debug)]
pub enum TaskWriteError {
    Service(Error),
}
impl From<Error> for TaskWriteError {
    fn from(error: Error) -> Self {
        Self::Service(error)
    }
}
impl fmt::Display for TaskWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Service(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for TaskWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Service(error) => Some(error),
        }
    }
}

impl VaultService {
    pub(in crate::services) fn retire_tasks(&self) -> Result<()> {
        let mut slot = self.tasks.lock().map_err(|_| Error::closed())?;
        *slot = None;
        Ok(())
    }
}

impl Client {
    /// One admitted operation; sibling projection crates are not part of this workspace.
    pub async fn control_tasks(&self, _command: NativeTaskCommand) -> Result<NativeTaskValue> {
        self.vault.lock()?.require_client(self.id)?;
        Err(Error::new(
            ErrorCode::Unsupported,
            "Native projection crates are not workspace path members",
        ))
    }
}
