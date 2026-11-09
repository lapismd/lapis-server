use super::{ContentSession, Result, failure};
use crate::services::{Client, Error};
use lapis_app_protocol::{ErrorCode, content::*};
use sha2::{Digest, Sha256};
use std::io::Read;

pub(super) fn native(error: Error) -> ContentError {
    let code = match error.code() {
        ErrorCode::NotFound => ContentErrorCode::NotFound,
        ErrorCode::PermissionDenied => ContentErrorCode::PermissionDenied,
        ErrorCode::Busy => ContentErrorCode::Busy,
        ErrorCode::Conflict => ContentErrorCode::Conflict,
        ErrorCode::InvalidRequest
        | ErrorCode::IsDirectory
        | ErrorCode::NotDirectory
        | ErrorCode::DirectoryNotEmpty => ContentErrorCode::InvalidRequest,
        _ => ContentErrorCode::Unavailable,
    };
    failure(code, "Content read is unavailable")
}

pub(super) fn io(error: std::io::Error) -> ContentError {
    native(error.into())
}

pub(super) fn read(session: &ContentSession, client: &Client, path: &str) -> Result<ContentValue> {
    let state = client.vault.lock().map_err(native)?;
    session.require_live()?;
    state.require_client(client.id).map_err(native)?;
    state.root.verify_physical_root().map_err(io)?;
    let source = state.root.open_file(path).map_err(io)?;
    let mut bytes = Vec::new();
    source
        .take(MAX_CONTENT_TEXT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes.len() > MAX_CONTENT_TEXT_BYTES {
        return Err(failure(
            ContentErrorCode::TooLarge,
            "Text exceeds the 1 MiB limit",
        ));
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| failure(ContentErrorCode::InvalidRequest, "Note is not UTF-8 text"))?;
    let version = format!("{:x}", Sha256::digest(text.as_bytes()));
    Ok(ContentValue::Note {
        path: path.into(),
        text,
        version,
    })
}

pub(super) fn list(
    session: &ContentSession,
    client: &Client,
    directory: &str,
) -> Result<Vec<ContentNoteSummary>> {
    let state = client.vault.lock().map_err(native)?;
    session.require_live()?;
    state.require_client(client.id).map_err(native)?;
    state.root.verify_physical_root().map_err(io)?;
    let listing = state.root.list(directory).map_err(io)?;
    let mut items = Vec::new();
    for name in listing.files {
        let extension = name.rsplit_once('.').map(|(_, extension)| extension);
        if !extension
            .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
        {
            continue;
        }
        let path = if directory.is_empty() {
            name
        } else {
            format!("{directory}/{name}")
        };
        if validate_content_path(&path).is_err() {
            continue;
        }
        let metadata = state.root.stat(&path).map_err(io)?.ok_or_else(|| {
            failure(
                ContentErrorCode::Conflict,
                "Listing changed; refresh the first page",
            )
        })?;
        if metadata.r#type != "file" {
            return Err(failure(
                ContentErrorCode::Conflict,
                "Listing changed; refresh the first page",
            ));
        }
        items.push(ContentNoteSummary {
            path,
            size_bytes: metadata.size,
        });
    }
    Ok(items)
}
