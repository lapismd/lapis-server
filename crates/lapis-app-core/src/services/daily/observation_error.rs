use super::DailyDocumentOperationError;
use crate::services::TaskWriteError;
use std::fmt;

#[derive(Debug)]
pub enum DefaultTaskObservationError {
    Provider(DailyDocumentOperationError),
    Write(TaskWriteError),
}
impl From<DailyDocumentOperationError> for DefaultTaskObservationError {
    fn from(error: DailyDocumentOperationError) -> Self {
        Self::Provider(error)
    }
}
impl From<TaskWriteError> for DefaultTaskObservationError {
    fn from(error: TaskWriteError) -> Self {
        Self::Write(error)
    }
}
impl fmt::Display for DefaultTaskObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(error) => error.fmt(formatter),
            Self::Write(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for DefaultTaskObservationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Provider(error) => error,
            Self::Write(error) => error,
        })
    }
}
