use super::{
    super::{
        State,
        errors::{Failure, blocking},
    },
    conditions, ranges,
};
use crate::host::Principal;
use axum::{
    body::Body,
    extract::Request,
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
};
use lapis_app_protocol::ErrorCode;
use std::io::SeekFrom;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

pub(super) async fn serve(
    state: State,
    id: Uuid,
    path: String,
    principal: Principal,
    request: Request,
    resource: bool,
) -> Result<Response, Failure> {
    let client = state.host.client(id, principal)?;
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    if resource
        && !matches!(mime.type_().as_str(), "image" | "audio" | "video")
        && mime.essence_str() != "application/pdf"
    {
        return Err(Failure(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            crate::host::error(ErrorCode::Unsupported, "Resource type is unavailable"),
        ));
    }
    let snapshot = blocking(move || client.snapshot(&path).map_err(|e| e.0)).await?;
    let etag = format!("\"{}\"", snapshot.version);
    if conditions::unmodified(request.headers(), &etag) {
        return Ok((StatusCode::NOT_MODIFIED, [("etag", etag)]).into_response());
    }
    let selected = match ranges::select(request.headers(), snapshot.length, &etag) {
        Ok(selected) => selected,
        Err(()) => {
            let mut response = Failure(
                StatusCode::RANGE_NOT_SATISFIABLE,
                crate::host::error(
                    ErrorCode::InvalidRequest,
                    "Requested byte range is unavailable",
                ),
            )
            .into_response();
            response.headers_mut().insert(
                "content-range",
                format!("bytes */{}", snapshot.length).parse().unwrap(),
            );
            return Ok(response);
        }
    };
    let mut response = Response::builder()
        .status(selected.status)
        .header("etag", etag)
        .header("accept-ranges", "bytes")
        .header("content-type", mime.to_string())
        .header("content-length", selected.length)
        .header(
            "cache-control",
            if resource {
                "private, max-age=0, must-revalidate"
            } else {
                "no-store"
            },
        )
        .header("content-security-policy", "default-src 'none'; sandbox");
    if !resource {
        response = response.header("content-disposition", "attachment");
    }
    if selected.status == StatusCode::PARTIAL_CONTENT {
        response = response.header(
            "content-range",
            format!(
                "bytes {}-{}/{}",
                selected.start,
                selected.start + selected.length - 1,
                snapshot.length
            ),
        );
    }
    let body = if request.method() == Method::HEAD {
        Body::empty()
    } else {
        let mut file = tokio::fs::File::from_std(snapshot.file);
        file.seek(SeekFrom::Start(selected.start))
            .await
            .map_err(|_| Failure::new(ErrorCode::Io, "File snapshot is unavailable"))?;
        Body::from_stream(ReaderStream::with_capacity(
            file.take(selected.length),
            64 * 1024,
        ))
    };
    response
        .body(body)
        .map_err(|_| Failure::new(ErrorCode::Io, "File response is unavailable"))
}
