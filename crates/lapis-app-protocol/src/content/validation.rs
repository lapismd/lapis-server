use super::*;
use uuid::Uuid;

fn invalid(message: &str) -> ContentError {
    ContentError {
        code: ContentErrorCode::InvalidRequest,
        message: message.into(),
        effect: ContentEffectStatus::NotStarted,
    }
}

fn id(value: Uuid) -> Result<(), ContentError> {
    if value.is_nil() {
        return Err(invalid("Identity must not be nil"));
    }
    Ok(())
}

/// Check a public content path lexically. Native physical checks remain required.
pub fn validate_content_path(value: &str) -> Result<(), ContentError> {
    if value.is_empty()
        || value.len() > MAX_CONTENT_PATH_BYTES
        || value.contains(['\\', ':'])
        || value.chars().any(char::is_control)
        || value.split('/').any(|part| {
            part.is_empty()
                || matches!(part, "." | "..")
                || part.ends_with(['.', ' '])
                || [".lapis", ".git", ".jj"]
                    .iter()
                    .any(|name| part.eq_ignore_ascii_case(name))
        })
    {
        return Err(invalid("Expected a public vault-relative path"));
    }
    Ok(())
}

fn page(value: &ContentPageQuery) -> Result<(), ContentError> {
    if value.limit == 0 || value.limit > MAX_CONTENT_PAGE_SIZE {
        return Err(invalid("Page limit must be between 1 and 100"));
    }
    if value.cursor.as_ref().is_some_and(|cursor| {
        cursor.is_empty()
            || cursor.len() > MAX_CONTENT_CURSOR_BYTES
            || !cursor.bytes().all(|byte| byte.is_ascii_graphic())
    }) {
        return Err(invalid("Invalid opaque cursor"));
    }
    Ok(())
}

fn text(value: &str) -> Result<(), ContentError> {
    if value.len() > MAX_CONTENT_TEXT_BYTES {
        return Err(ContentError {
            code: ContentErrorCode::TooLarge,
            message: "Text exceeds the 1 MiB limit".into(),
            effect: ContentEffectStatus::NotStarted,
        });
    }
    Ok(())
}

impl ContentRequest {
    /// Validate semantic input limits after bounded transport decoding and before
    /// admission. This never authenticates a caller or proves physical path safety.
    pub fn validate(&self) -> Result<(), ContentError> {
        if self.api_version != CONTENT_API_MAJOR {
            return Err(ContentError {
                code: ContentErrorCode::ProtocolMismatch,
                message: "Unsupported content API major".into(),
                effect: ContentEffectStatus::NotStarted,
            });
        }
        id(self.request_id)?;
        match &self.command {
            ContentCommand::Capabilities {} => Ok(()),
            ContentCommand::ListVaults { page: query } => page(query),
            ContentCommand::ListNotes {
                vault_id,
                directory,
                page: query,
            } => {
                id(*vault_id)?;
                if let Some(directory) = directory {
                    validate_content_path(directory)?;
                }
                page(query)
            }
            ContentCommand::ReadNote {
                vault_id,
                path: note,
            } => {
                id(*vault_id)?;
                validate_content_path(note)
            }
            ContentCommand::CreateNote {
                vault_id,
                path: note,
                text: body,
            } => {
                id(*vault_id)?;
                validate_content_path(note)?;
                text(body)
            }
            ContentCommand::ReplaceNote {
                vault_id,
                path: note,
                text: body,
                expected_version,
            } => {
                id(*vault_id)?;
                validate_content_path(note)?;
                text(body)?;
                if expected_version.is_empty()
                    || expected_version == "*"
                    || expected_version.starts_with("W/")
                    || expected_version.len() > MAX_CONTENT_VERSION_BYTES
                    || !expected_version.bytes().all(|byte| byte.is_ascii_graphic())
                {
                    return Err(invalid("Expected a strong opaque content version"));
                }
                Ok(())
            }
        }
    }
}
