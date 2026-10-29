use crate::vault::TextWriteConflict;
use lapis_app_protocol::{ErrorCode, ServiceError};
use std::{fmt, io};

#[derive(Debug)]
pub struct Error(pub ServiceError);
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn code(&self) -> ErrorCode {
        self.0.code
    }
    pub(crate) fn new(code: ErrorCode, message: &str) -> Self {
        Self(ServiceError {
            code,
            message: message.into(),
        })
    }
    pub(super) fn closed() -> Self {
        Self::new(ErrorCode::Closed, "Vault service is closed")
    }
    pub(super) fn busy() -> Self {
        Self::new(ErrorCode::Busy, "Vault has unresolved work or editor state")
    }
    pub(super) fn denied() -> Self {
        Self::new(ErrorCode::PermissionDenied, "Vault attachment is not valid")
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.message.fmt(f)
    }
}
impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        if error
            .get_ref()
            .is_some_and(|cause| cause.is::<TextWriteConflict>())
        {
            return Self::new(ErrorCode::Conflict, "File changed since it was read");
        }
        match error.kind() {
            io::ErrorKind::AlreadyExists => {
                Self::new(ErrorCode::AlreadyExists, "File already exists")
            }
            io::ErrorKind::DirectoryNotEmpty => {
                Self::new(ErrorCode::DirectoryNotEmpty, "Directory is not empty")
            }
            io::ErrorKind::IsADirectory => {
                Self::new(ErrorCode::IsDirectory, "Operation requires a file")
            }
            io::ErrorKind::NotADirectory => {
                Self::new(ErrorCode::NotDirectory, "Operation requires a directory")
            }
            io::ErrorKind::Unsupported => {
                Self::new(ErrorCode::Unsupported, "Operation is unavailable")
            }
            io::ErrorKind::NotFound => Self::new(ErrorCode::NotFound, "File does not exist"),
            io::ErrorKind::PermissionDenied => Self::new(
                ErrorCode::PermissionDenied,
                "File operation permission denied",
            ),
            io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => {
                Self::new(ErrorCode::InvalidRequest, "Invalid vault operation")
            }
            _ => Self::new(ErrorCode::Io, "Vault operation failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filesystem_permission_denial_is_distinct_from_attachment_denial() {
        let failure = Error::from(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "/private/vault/Attachments is not writable",
        ));
        assert_eq!(failure.code(), ErrorCode::PermissionDenied);
        assert!(failure.to_string().to_lowercase().contains("permission"));
        assert!(!failure.to_string().contains("/private/"));
        assert_eq!(Error::denied().to_string(), "Vault attachment is not valid");
    }
}
