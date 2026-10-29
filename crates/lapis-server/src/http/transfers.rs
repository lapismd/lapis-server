use super::{
    State,
    errors::{Failure, blocking},
};
use crate::host::ManagementCommand;
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, Request, State as AxumState},
    http::{Method, StatusCode, Version},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use lapis_app_core::archive::ArchiveLimits;
use lapis_app_protocol::ErrorCode;
use serde::Deserialize;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ImportName {
    name: String,
}

pub(super) async fn import(
    AxumState(state): AxumState<State>,
    Query(query): Query<ImportName>,
    request: Request,
) -> Response {
    let version = request.version();
    match receive(state, query.name, request).await {
        Ok(response) => response,
        Err(failure) => {
            let mut response = failure.into_response();
            if matches!(version, Version::HTTP_10 | Version::HTTP_11) {
                response
                    .headers_mut()
                    .insert("connection", "close".parse().unwrap());
            }
            response
        }
    }
}
async fn receive(state: State, name: String, request: Request) -> Result<Response, Failure> {
    if !state.host.manages_vaults() {
        return Err(Failure::new(
            ErrorCode::Unsupported,
            "Vault transfers require the installation profile",
        ));
    }
    let limit = ArchiveLimits::default().archive_bytes;
    if request
        .headers()
        .get("content-length")
        .is_some_and(|value| {
            value
                .to_str()
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .is_none_or(|length| length > limit)
        })
    {
        return Err(too_large());
    }
    let file = blocking(|| {
        tempfile::tempfile()
            .map_err(|_| crate::host::error(ErrorCode::Io, "Archive staging is unavailable"))
    })
    .await?;
    let mut file = tokio::fs::File::from_std(file);
    let mut body = request.into_body().into_data_stream();
    let mut length = 0u64;
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|_| {
            Failure::new(ErrorCode::InvalidRequest, "Archive upload was interrupted")
        })?;
        length = length.saturating_add(chunk.len() as u64);
        if length > limit {
            return Err(too_large());
        }
        file.write_all(&chunk).await.map_err(staging)?;
    }
    file.flush().await.map_err(staging)?;
    file.rewind().await.map_err(staging)?;
    let file = file.into_std().await;
    // Nothing is accepted or published until the entire upload has arrived.
    let vault = state
        .host
        .manage(ManagementCommand::Import(name, file))
        .await?;
    Ok((StatusCode::CREATED, Json(vault)).into_response())
}

pub(super) async fn export(
    AxumState(state): AxumState<State>,
    Path(id): Path<Uuid>,
    request: Request,
) -> Result<Response, Failure> {
    let snapshot = state.host.archive_snapshot(id).await?;
    let body = if request.method() == Method::HEAD {
        Body::empty()
    } else {
        Body::from_stream(ReaderStream::with_capacity(
            tokio::fs::File::from_std(snapshot.file),
            64 * 1024,
        ))
    };
    Response::builder()
        .header("content-type", "application/zip")
        .header(
            "content-disposition",
            format!("attachment; filename=\"lapis-{id}.zip\""),
        )
        .header("content-length", snapshot.length)
        .header("cache-control", "no-store")
        .header("x-content-type-options", "nosniff")
        .body(body)
        .map_err(|_| Failure::new(ErrorCode::Io, "Vault export response is unavailable"))
}
fn too_large() -> Failure {
    Failure(
        StatusCode::PAYLOAD_TOO_LARGE,
        crate::host::error(
            ErrorCode::InvalidRequest,
            "ZIP exceeds the 512 MiB compressed limit",
        ),
    )
}
fn staging(_: std::io::Error) -> Failure {
    Failure::new(ErrorCode::Io, "Archive upload staging failed")
}
