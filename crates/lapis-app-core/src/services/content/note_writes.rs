use super::{Result, failure};
use crate::services::{Client, Error, plugin_operations::OperationProvider};
use lapis_app_protocol::{
    ErrorCode, ExpectedVersion,
    content::{ContentEffectStatus, ContentError, ContentErrorCode, validate_content_path},
    operations::{OperationDescriptor, OperationRequest},
};
use serde_json::{Value, json};

const MAX_NOTE_WRITE_BYTES: usize = 48 * 1024;

pub struct NoteWrites;

impl OperationProvider for NoteWrites {
    fn descriptors(&self) -> Vec<OperationDescriptor> {
        serde_json::from_str(include_str!("../../../notes/operations.json"))
            .expect("valid bundled note write descriptors")
    }

    fn available(&self, _client: &Client) -> Result<bool> {
        Ok(true)
    }

    fn invoke(&self, client: &Client, id: &str, request: &OperationRequest) -> Result<Value> {
        if id == "search" {
            return super::note_search::search(client, &request.input);
        }
        let path = string(&request.input, "path")?;
        validate_note_path(path)?;
        match id {
            "frontmatter-get" => return super::frontmatter::read(client, path),
            "frontmatter-patch" => return super::frontmatter::patch(client, path, &request.input),
            _ => {}
        }
        let text = string(&request.input, "text")?;
        if text.len() > MAX_NOTE_WRITE_BYTES {
            return Err(failure(
                ContentErrorCode::TooLarge,
                "Note exceeds the 48 KiB write limit",
            ));
        }
        let expected = match id {
            "create" => ExpectedVersion::Missing {},
            "replace" => ExpectedVersion::Version {
                version: string(&request.input, "expected_version")?.into(),
            },
            _ => {
                return Err(failure(
                    ContentErrorCode::Unsupported,
                    "Unsupported note operation",
                ));
            }
        };
        let version = client
            .write_text(path, text, &expected)
            .map_err(write_error)?;
        Ok(json!({ "path": path, "version": version }))
    }
}

pub(super) fn validate_note_path(path: &str) -> Result<()> {
    validate_content_path(path)?;
    let markdown = path.rsplit_once('.').is_some_and(|(_, suffix)| {
        suffix.eq_ignore_ascii_case("md") || suffix.eq_ignore_ascii_case("markdown")
    });
    if !markdown || path.split('/').any(|part| part.starts_with('.')) {
        return Err(failure(
            ContentErrorCode::InvalidRequest,
            "Expected a public Markdown note path",
        ));
    }
    Ok(())
}

fn string<'a>(input: &'a Value, field: &str) -> Result<&'a str> {
    input[field].as_str().ok_or_else(|| {
        failure(
            ContentErrorCode::InvalidRequest,
            "Invalid note operation input",
        )
    })
}

pub(super) fn write_error(error: Error) -> ContentError {
    let (code, message) = match error.code() {
        ErrorCode::Busy => (ContentErrorCode::Busy, "An editor or native writer is busy"),
        ErrorCode::Conflict | ErrorCode::AlreadyExists => (
            ContentErrorCode::Conflict,
            "The note changed or already exists",
        ),
        ErrorCode::NotFound => (
            ContentErrorCode::NotFound,
            "The note or its parent folder is missing",
        ),
        ErrorCode::PermissionDenied => (
            ContentErrorCode::PermissionDenied,
            "The note path is not admitted",
        ),
        ErrorCode::InvalidRequest | ErrorCode::IsDirectory | ErrorCode::NotDirectory => (
            ContentErrorCode::InvalidRequest,
            "Invalid note path or version",
        ),
        ErrorCode::Io => {
            return ContentError {
                code: ContentErrorCode::OutcomeUnknown,
                message: "The note write was interrupted; read the saved note before retrying"
                    .into(),
                effect: ContentEffectStatus::Unknown,
            };
        }
        _ => (ContentErrorCode::Unavailable, "Note writing is unavailable"),
    };
    failure(code, message)
}
