use crate::errors::failure;
use lapis_app_protocol::content::*;
use uuid::Uuid;

pub(crate) fn version(etag: &str) -> Result<String, ContentError> {
    let value = etag
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .filter(|v| {
            !v.is_empty()
                && *v != "*"
                && !v.starts_with("W/")
                && v.len() <= 256
                && v.bytes().all(|b| b.is_ascii_graphic() && b != b'"')
        })
        .ok_or_else(|| failure(ContentErrorCode::ProtocolMismatch))?;
    Ok(value.into())
}
pub(crate) fn value(value: &ContentValue, command: &ContentCommand) -> Result<(), ContentError> {
    let valid = match (value, command) {
        (ContentValue::Capabilities { capabilities }, ContentCommand::Capabilities {}) => {
            capabilities.api_version == CONTENT_API_MAJOR
                && capabilities.operations.len() <= CONTENT_OPERATIONS.len()
                && capabilities.operations.iter().all(|op| {
                    let expected = op.operation.descriptor();
                    op.scope == expected.scope && op.effect == expected.effect
                })
        }
        (ContentValue::Vaults { items, next_cursor }, ContentCommand::ListVaults { page }) => {
            cursor(next_cursor)
                && items.len() <= page.limit as usize
                && items.iter().all(|v| {
                    !v.vault_id.is_nil()
                        && !v.name.trim().is_empty()
                        && v.name.len() <= 256
                        && !v.name.chars().any(char::is_control)
                })
        }
        (ContentValue::Notes { items, next_cursor }, ContentCommand::ListNotes { page, .. }) => {
            cursor(next_cursor)
                && items.len() <= page.limit as usize
                && items
                    .iter()
                    .all(|note| validate_content_path(&note.path).is_ok())
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(failure(ContentErrorCode::ProtocolMismatch))
    }
}
fn cursor(cursor: &Option<String>) -> bool {
    ContentRequest {
        api_version: CONTENT_API_MAJOR,
        request_id: Uuid::new_v4(),
        command: ContentCommand::ListVaults {
            page: ContentPageQuery {
                limit: 1,
                cursor: cursor.clone(),
            },
        },
    }
    .validate()
    .is_ok()
}
