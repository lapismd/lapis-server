use super::State;
use crate::host::Principal;
use axum::{
    Extension, Json,
    extract::{Path, State as AxumState, rejection::StringRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use lapis_app_core::services::{HistoryError, HistoryMethod};
use uuid::Uuid;

pub(super) async fn call(
    AxumState(state): AxumState<State>,
    Path((client, method)): Path<(Uuid, String)>,
    Extension(principal): Extension<Principal>,
    frame: Result<String, StringRejection>,
) -> Result<impl IntoResponse, Failure> {
    let frame = frame.map_err(|error| {
        Failure(if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
            HistoryError::Limit
        } else {
            HistoryError::Invalid
        })
    })?;
    let method: HistoryMethod = method.parse().map_err(Failure)?;
    state
        .host
        .read_history(client, principal, method, frame)
        .await
        .map(|text| ([("content-type", "application/json")], text))
        .map_err(Failure)
}
pub(super) struct Failure(pub(super) HistoryError);
impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        use HistoryError::*;
        let (status, code) = match &self.0 {
            Denied => (StatusCode::FORBIDDEN, "PERMISSION_DENIED"),
            Invalid | InvalidQuery(_) | Range => (StatusCode::BAD_REQUEST, "INVALID_REQUEST"),
            Limit => (StatusCode::PAYLOAD_TOO_LARGE, "INVALID_REQUEST"),
            NotFound => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            Canceled | BlameReset | EpochChanged | Busy | IndexBudget | IndexRebuildRequired => {
                (StatusCode::CONFLICT, "CONFLICT")
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "IO"),
        };
        let mut response = (
            status,
            Json(serde_json::json!({
                "code": code, "message": self.0.to_string(), "history": self.0
            })),
        )
            .into_response();
        if status == StatusCode::PAYLOAD_TOO_LARGE {
            response
                .headers_mut()
                .insert("connection", "close".parse().unwrap());
        }
        response
    }
}
