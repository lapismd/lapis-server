use super::{Error, Result, state::State};
use crate::vault::{MAX_FILE_BYTES, WriteOptions};
use lapis_app_protocol::{ErrorCode, ExpectedVersion, TextSnapshot};
use sha2::{Digest, Sha256};

pub(super) fn read(state: &State, path: &str) -> Result<TextSnapshot> {
    state.root.verify_physical_root()?;
    let text = state.root.read_text(path)?;
    let version = format!("{:x}", Sha256::digest(text.as_bytes()));
    Ok(TextSnapshot {
        path: path.into(),
        text,
        version,
    })
}
pub(super) fn write(
    state: &State,
    path: &str,
    text: &str,
    expected: &ExpectedVersion,
    writer: Option<uuid::Uuid>,
) -> Result<String> {
    state.root.verify_physical_root()?;
    if text.len() as u64 > MAX_FILE_BYTES {
        return Err(Error::new(
            ErrorCode::InvalidRequest,
            "Text exceeds the file limit",
        ));
    }
    super::byte_writes::write(
        state,
        path,
        text.as_bytes(),
        Some(expected),
        &WriteOptions::default(),
        writer,
    )
}

/// Validate an explicit existing-file write, including when its result is a no-op.
pub(super) fn guard_existing_write(
    state: &State,
    path: &str,
    expected: &ExpectedVersion,
    writer: Option<uuid::Uuid>,
) -> Result<TextSnapshot> {
    state.root.verify_physical_root()?;
    super::guards::write(state, path, writer, false)?;
    let ExpectedVersion::Version { version } = expected else {
        return Err(Error::new(
            ErrorCode::InvalidRequest,
            "Existing content version is required",
        ));
    };
    let bytes = super::byte_writes::read_versioned(state, path, version)?;
    let text = String::from_utf8(bytes)
        .map_err(|_| Error::new(ErrorCode::InvalidRequest, "Invalid vault operation"))?;
    Ok(TextSnapshot {
        path: path.into(),
        text,
        version: version.clone(),
    })
}

pub(super) fn write_if_changed(
    state: &State,
    path: &str,
    text: &str,
    expected: &ExpectedVersion,
    writer: Option<uuid::Uuid>,
) -> Result<String> {
    let current = guard_existing_write(state, path, expected, writer)?;
    if current.text == text {
        return Ok(current.version);
    }
    write(state, path, text, expected, writer)
}
