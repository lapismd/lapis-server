use super::{
    super::{
        State,
        errors::{Failure, blocking},
    },
    conditions,
};
use crate::host::Principal;
use axum::{
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use lapis_app_core::vault::MAX_FILE_BYTES;
use lapis_app_protocol::ErrorCode;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use uuid::Uuid;

pub(super) async fn receive(
    state: State,
    id: Uuid,
    path: String,
    principal: Principal,
    request: Request,
) -> Result<Response, Failure> {
    let client = state.host.client(id, principal)?;
    let expected = conditions::write(request.headers())?;
    let options = conditions::options(request.headers())?;
    if request
        .headers()
        .get("content-length")
        .is_some_and(|value| {
            value
                .to_str()
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .is_none_or(|length| length > MAX_FILE_BYTES)
        })
    {
        return Err(too_large());
    }
    let file = blocking(|| {
        tempfile::tempfile()
            .map_err(|_| crate::host::error(ErrorCode::Io, "File upload staging is unavailable"))
    })
    .await?;
    let mut file = tokio::fs::File::from_std(file);
    let mut body = request.into_body().into_data_stream();
    let mut length = 0;
    while let Some(chunk) = body.next().await {
        let chunk = chunk
            .map_err(|_| Failure::new(ErrorCode::InvalidRequest, "File body was interrupted"))?;
        length += chunk.len() as u64;
        if length > MAX_FILE_BYTES {
            return Err(too_large());
        }
        file.write_all(&chunk).await.map_err(staging_error)?;
    }
    file.flush().await.map_err(staging_error)?;
    file.rewind().await.map_err(staging_error)?;
    let file = file.into_std().await;
    let version = blocking(move || {
        client
            .write_from(&path, file, expected.as_ref(), &options)
            .map_err(|e| e.0)
    })
    .await
    .map_err(|mut failure| {
        if failure.1.code == ErrorCode::Conflict {
            failure.0 = StatusCode::PRECONDITION_FAILED;
        }
        failure
    })?;
    Ok((StatusCode::NO_CONTENT, [("etag", format!("\"{version}\""))]).into_response())
}
fn too_large() -> Failure {
    Failure(
        StatusCode::PAYLOAD_TOO_LARGE,
        crate::host::error(
            ErrorCode::InvalidRequest,
            "File body exceeds the 64 MiB limit",
        ),
    )
}
fn staging_error(_: std::io::Error) -> Failure {
    Failure::new(ErrorCode::Io, "File upload staging failed")
}
