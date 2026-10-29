use super::super::{State, errors::Failure};
use crate::host::Principal;
use axum::{
    Extension,
    body::Body,
    extract::{Path, Query, Request, State as AxumState},
    http::{Method, StatusCode},
    response::Response,
};
use futures_util::StreamExt;
use lapis_app_protocol::ErrorCode;
use serde::Deserialize;
use std::time::Duration;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

#[cfg(test)]
mod tests;

const MAX_BYTES: u64 = 64 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::http) struct Source {
    source: String,
}

pub(in crate::http) fn client() -> std::io::Result<reqwest::Client> {
    reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 10 || !redirect_allowed(attempt.url()) {
                attempt.error("Plugin bundle redirect is not permitted")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| std::io::Error::other("Plugin download transport is unavailable"))
}
fn redirect_allowed(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}
fn official(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    if url.scheme() != "https"
        || !matches!(url.host_str(), Some("github.com" | "registry.lapis.md"))
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return false;
    }
    let parts: Vec<_> = url.path().split('/').collect();
    if url.host_str() == Some("registry.lapis.md") {
        return matches!(parts.as_slice(), ["", "download", plugin, version]
            if !plugin.is_empty() && plugin.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-') && !version.is_empty());
    }
    matches!(parts.as_slice(), ["", org, repo, "releases", "download", tag, file]
        if matches!((*org,*repo), ("lapismd","lapis-plugins") | ("lapis-notes","releases"))
            && !tag.is_empty() && !file.is_empty() && file.ends_with(".lapis-plugin"))
}

pub(in crate::http) async fn download(
    AxumState(state): AxumState<State>,
    Path(id): Path<Uuid>,
    Extension(principal): Extension<Principal>,
    Query(source): Query<Source>,
    request: Request,
) -> Result<Response, Failure> {
    state.host.client(id, principal)?;
    if !official(&source.source) {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Plugin bundle source is not permitted",
        ));
    }
    // Build the upstream request from public coordinates only. No caller headers,
    // cookies, authorization or native paths cross this boundary.
    let upstream = state
        .downloads
        .request(request.method().clone(), source.source)
        .header("accept", "application/octet-stream")
        .send()
        .await
        .map_err(|_| unavailable())?;
    if !upstream.status().is_success() {
        return Err(unavailable());
    }
    if upstream
        .content_length()
        .is_some_and(|size| size > MAX_BYTES)
    {
        return Err(too_large());
    }
    let mut response = Response::builder()
        .header("content-type", "application/octet-stream")
        .header("cache-control", "private, no-store")
        .header("x-content-type-options", "nosniff");
    if request.method() == Method::HEAD {
        if let Some(length) = upstream.content_length() {
            response = response.header("content-length", length);
        }
        return response.body(Body::empty()).map_err(|_| unavailable());
    }
    let snapshot = bounded(upstream, MAX_BYTES).await?;
    response
        .header("content-length", snapshot.length)
        .body(Body::from_stream(ReaderStream::with_capacity(
            snapshot.file,
            64 * 1024,
        )))
        .map_err(|_| unavailable())
}
struct Download {
    file: tokio::fs::File,
    length: u64,
}
async fn bounded(response: reqwest::Response, limit: u64) -> Result<Download, Failure> {
    if response
        .content_length()
        .is_some_and(|length| length > limit)
    {
        return Err(too_large());
    }
    let file = tokio::task::spawn_blocking(tempfile::tempfile)
        .await
        .map_err(|_| unavailable())?
        .map_err(|_| unavailable())?;
    let mut file = tokio::fs::File::from_std(file);
    let mut stream = response.bytes_stream();
    let mut length = 0u64;
    while let Some(bytes) = stream.next().await {
        let bytes = bytes.map_err(|_| unavailable())?;
        length = length.saturating_add(bytes.len() as u64);
        if length > limit {
            return Err(too_large());
        }
        file.write_all(&bytes).await.map_err(|_| unavailable())?;
    }
    file.flush().await.map_err(|_| unavailable())?;
    file.rewind().await.map_err(|_| unavailable())?;
    Ok(Download { file, length })
}
fn unavailable() -> Failure {
    Failure::new(ErrorCode::NotFound, "Plugin bundle download failed")
}
fn too_large() -> Failure {
    Failure(
        StatusCode::PAYLOAD_TOO_LARGE,
        crate::host::error(
            ErrorCode::InvalidRequest,
            "Plugin bundle response is too large",
        ),
    )
}
