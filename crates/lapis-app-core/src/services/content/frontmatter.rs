use super::{Result, failure, note_writes::write_error, reads};
use crate::services::{Client, operations};
use lapis_app_protocol::{
    ExpectedVersion,
    content::{ContentErrorCode, MAX_CONTENT_TEXT_BYTES},
};
use lapis_plugin_services::markdown::{FrontmatterError, patch_frontmatter, read_frontmatter};
use serde::Deserialize;
use serde_json::{Map, Value, json};

pub(super) fn read(client: &Client, path: &str) -> Result<Value> {
    let snapshot = client.read_text(path).map_err(reads::native)?;
    check_note_size(&snapshot.text)?;
    let properties = read_frontmatter(&snapshot.text).map_err(yaml_error)?;
    Ok(json!({ "path": path, "version": snapshot.version, "properties": properties }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Patch {
    path: String,
    expected_version: String,
    #[serde(default)]
    set: Map<String, Value>,
    #[serde(default)]
    remove: Vec<String>,
}

pub(super) fn patch(client: &Client, path: &str, input: &Value) -> Result<Value> {
    let patch: Patch = serde_json::from_value(input.clone()).map_err(|_| {
        failure(
            ContentErrorCode::InvalidRequest,
            "Invalid frontmatter patch",
        )
    })?;
    debug_assert_eq!(patch.path, path);
    if patch.set.is_empty() && patch.remove.is_empty() {
        return Err(failure(
            ContentErrorCode::InvalidRequest,
            "Supply properties to set or remove",
        ));
    }
    let expected = ExpectedVersion::Version {
        version: patch.expected_version,
    };
    let state = client.vault.lock().map_err(reads::native)?;
    state.require_client(client.id).map_err(reads::native)?;
    let snapshot = operations::guard_existing_write(&state, path, &expected, Some(client.id))
        .map_err(reads::native)?;
    check_note_size(&snapshot.text)?;
    let text = patch_frontmatter(&snapshot.text, &patch.set, &patch.remove).map_err(yaml_error)?;
    check_note_size(&text)?;
    let changed = text != snapshot.text;
    let version = if changed {
        operations::write(&state, path, &text, &expected, Some(client.id)).map_err(write_error)?
    } else {
        snapshot.version
    };
    Ok(json!({ "path": path, "version": version, "changed": changed }))
}

fn check_note_size(text: &str) -> Result<()> {
    if text.len() <= MAX_CONTENT_TEXT_BYTES {
        Ok(())
    } else {
        Err(failure(
            ContentErrorCode::TooLarge,
            "Note exceeds the 1 MiB content limit",
        ))
    }
}

fn yaml_error(error: FrontmatterError) -> lapis_app_protocol::content::ContentError {
    let code = match error {
        FrontmatterError::Invalid => ContentErrorCode::InvalidRequest,
        FrontmatterError::UnsupportedLayout => ContentErrorCode::Unsupported,
    };
    failure(code, &error.to_string())
}
