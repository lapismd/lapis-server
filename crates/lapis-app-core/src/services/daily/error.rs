use serde::Serialize;
use std::fmt;

#[derive(Clone, Debug, Serialize)]
pub struct DailyDocumentError {
    name: &'static str,
    pub message: String,
}
impl DailyDocumentError {
    pub(in crate::services) fn new(message: impl Into<String>) -> Self {
        Self {
            name: "Error",
            message: message.into(),
        }
    }
}
impl fmt::Display for DailyDocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for DailyDocumentError {}

#[derive(Debug)]
pub enum DailyDocumentOperationError {
    Service(crate::services::Error),
    Policy(DailyDocumentError),
}
pub type DailyDocumentOperationResult<T> = Result<T, DailyDocumentOperationError>;
impl From<crate::services::Error> for DailyDocumentOperationError {
    fn from(error: crate::services::Error) -> Self {
        Self::Service(error)
    }
}
impl From<std::io::Error> for DailyDocumentOperationError {
    fn from(error: std::io::Error) -> Self {
        Self::Service(error.into())
    }
}
impl From<DailyDocumentError> for DailyDocumentOperationError {
    fn from(error: DailyDocumentError) -> Self {
        Self::Policy(error)
    }
}
impl fmt::Display for DailyDocumentOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Service(error) => error.fmt(formatter),
            Self::Policy(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for DailyDocumentOperationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Service(error) => error,
            Self::Policy(error) => error,
        })
    }
}
