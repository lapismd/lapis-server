use super::{Asset, paths};
use crate::http::{State, files::conditions};
use axum::{
    body::Body,
    extract::{Request, State as AxumState},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
};
use std::io;
use tokio_util::io::ReaderStream;

pub(in crate::http) async fn serve(
    AxumState(state): AxumState<State>,
    request: Request,
) -> Response {
    let path = match paths::decode(request.uri().path()) {
        Ok(path) => path,
        Err(status) => return (status, "Invalid static path").into_response(),
    };
    if paths::reserved(&path) {
        return StatusCode::NOT_FOUND.into_response();
    }
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        return (StatusCode::METHOD_NOT_ALLOWED, [("allow", "GET, HEAD")]).into_response();
    }
    let Some(web) = state.web else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let selected = tokio::task::spawn_blocking(move || -> io::Result<(Asset, String)> {
        let path = if path.is_empty() {
            "index.html".into()
        } else {
            path
        };
        match web.asset(&path) {
            Ok(asset) => Ok((asset, path)),
            Err(error) if error.kind() == io::ErrorKind::NotFound && paths::navigation(&path) => {
                Ok((web.asset("index.html")?, "index.html".into()))
            }
            Err(error) => Err(error),
        }
    })
    .await;
    let (asset, path) = match selected {
        Ok(Ok(asset)) => asset,
        Ok(Err(error))
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound
                    | io::ErrorKind::PermissionDenied
                    | io::ErrorKind::InvalidInput
            ) =>
        {
            return StatusCode::NOT_FOUND.into_response();
        }
        _ => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Static build is unavailable",
            )
                .into_response();
        }
    };
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    let mime = if mime.type_().as_str() == "text" {
        format!("{mime}; charset=utf-8")
    } else {
        mime.to_string()
    };
    let unchanged = conditions::unmodified(request.headers(), &asset.etag);
    let response = Response::builder()
        .status(if unchanged {
            StatusCode::NOT_MODIFIED
        } else {
            StatusCode::OK
        })
        .header("content-type", mime)
        .header("content-length", asset.length)
        .header("etag", asset.etag)
        .header("cache-control", paths::cache(&path))
        .header("cross-origin-opener-policy", "same-origin")
        .header("cross-origin-embedder-policy", "require-corp")
        .header("cross-origin-resource-policy", "same-origin")
        .header("x-content-type-options", "nosniff");
    let body = if unchanged || request.method() == Method::HEAD {
        Body::empty()
    } else {
        Body::from_stream(ReaderStream::with_capacity(
            tokio::fs::File::from_std(asset.file),
            64 * 1024,
        ))
    };
    response
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
