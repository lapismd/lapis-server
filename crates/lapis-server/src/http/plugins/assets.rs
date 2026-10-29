use super::super::{
    State,
    errors::{Failure, blocking},
};
use crate::host::Principal;
use axum::{
    Extension,
    body::Body,
    extract::{Path, Request, State as AxumState},
    http::{Method, StatusCode},
    response::Response,
};
use lapis_app_protocol::ErrorCode;
use uuid::Uuid;

pub(in crate::http) async fn serve(
    AxumState(state): AxumState<State>,
    Path((id, plugin, version, fingerprint, path)): Path<(Uuid, String, String, String, String)>,
    Extension(principal): Extension<Principal>,
    request: Request,
) -> Result<Response, Failure> {
    if request.uri().query().is_some() {
        return Err(unavailable());
    }
    let client = state.host.client(id, principal)?;
    let asset = blocking(move || {
        client
            .plugin_asset(&plugin, &version, &fingerprint, &path)
            .map_err(|_| crate::host::error(ErrorCode::NotFound, "Plugin asset unavailable"))
    })
    .await?;
    let etag = format!("\"{}\"", asset.sha256);
    let mut response = Response::builder()
        .header("content-type", asset.content_type)
        .header("cache-control", "no-store")
        .header("etag", &etag)
        .header("accept-ranges", "bytes")
        .header("cross-origin-resource-policy", "same-origin")
        .header("x-lapis-plugin-asset-sha256", asset.sha256);
    // Verify the current installation and bytes even for a conditional request.
    if request
        .headers()
        .get("if-none-match")
        .is_some_and(|header| header.as_bytes() == etag.as_bytes())
    {
        return response
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())
            .map_err(|_| unavailable());
    }
    let selected =
        super::super::files::ranges::select(request.headers(), asset.bytes.len() as u64, &etag)
            .map_err(|_| {
                Failure(
                    StatusCode::RANGE_NOT_SATISFIABLE,
                    crate::host::error(
                        ErrorCode::InvalidRequest,
                        "Requested plugin byte range is unavailable",
                    ),
                )
            })?;
    response = response
        .status(selected.status)
        .header("content-length", selected.length);
    if selected.status == StatusCode::PARTIAL_CONTENT {
        response = response.header(
            "content-range",
            format!(
                "bytes {}-{}/{}",
                selected.start,
                selected.start + selected.length - 1,
                asset.bytes.len()
            ),
        );
    }
    let body = if request.method() == Method::HEAD {
        Body::empty()
    } else {
        let bytes = axum::body::Bytes::from(asset.bytes);
        Body::from(
            bytes.slice(selected.start as usize..(selected.start + selected.length) as usize),
        )
    };
    response.body(body).map_err(|_| unavailable())
}
fn unavailable() -> Failure {
    Failure::new(ErrorCode::NotFound, "Plugin asset unavailable")
}
