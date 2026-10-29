use crate::host::error;
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use lapis_app_protocol::{ErrorCode, ServiceError};

pub(super) struct Failure(pub StatusCode, pub ServiceError);
pub(super) async fn blocking<T: Send + 'static>(
    action: impl FnOnce() -> crate::host::Result<T> + Send + 'static,
) -> Result<T, Failure> {
    tokio::task::spawn_blocking(action)
        .await
        .map_err(|_| Failure::new(ErrorCode::Io, "Host operation did not complete"))?
        .map_err(Into::into)
}
impl Failure {
    pub fn new(code: ErrorCode, message: &str) -> Self {
        error(code, message).into()
    }
    pub fn unauthorized() -> Self {
        Self(
            StatusCode::UNAUTHORIZED,
            error(
                ErrorCode::PermissionDenied,
                "Owner authentication is required",
            ),
        )
    }
}
impl From<ServiceError> for Failure {
    fn from(error: ServiceError) -> Self {
        let status = match error.code {
            ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            ErrorCode::ProtocolMismatch
            | ErrorCode::Conflict
            | ErrorCode::Busy
            | ErrorCode::AlreadyExists
            | ErrorCode::DirectoryNotEmpty => StatusCode::CONFLICT,
            ErrorCode::PermissionDenied => StatusCode::FORBIDDEN,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::Closed => StatusCode::GONE,
            ErrorCode::Unsupported => StatusCode::NOT_IMPLEMENTED,
            ErrorCode::IsDirectory | ErrorCode::NotDirectory => StatusCode::BAD_REQUEST,
            ErrorCode::Io => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self(status, error)
    }
}
impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        let mut response = (self.0, Json(self.1)).into_response();
        // This host serves HTTP/1. An early size rejection can leave body bytes
        // unread; tell clients to retire it before their next admitted command.
        if self.0 == StatusCode::PAYLOAD_TOO_LARGE {
            response
                .headers_mut()
                .insert("connection", "close".parse().unwrap());
        }
        response
    }
}
