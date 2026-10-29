use crate::services::{Error, Result};
use lapis_app_protocol::{ErrorCode, native_tasks::*};

fn invalid() -> Error {
    Error::new(
        ErrorCode::InvalidRequest,
        "Invalid bounded native Task command",
    )
}
fn path(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 4096
        || value.contains(['\\', ':', '\0'])
        || value
            .split('/')
            .any(|part| part.is_empty() || part.starts_with('.'))
        || !crate::services::indexing::records::markdown(value)
    {
        return Err(invalid());
    }
    Ok(())
}
fn version(value: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|value| value.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    Ok(())
}
fn id(value: &uuid::Uuid) -> Result<()> {
    if value.get_version_num() != 4 {
        return Err(invalid());
    }
    Ok(())
}
pub(super) fn command(value: &NativeTaskCommand) -> Result<()> {
    if serde_json::to_vec(value).map_err(|_| invalid())?.len() > 64 * 1024 {
        return Err(invalid());
    }
    match value {
        NativeTaskCommand::Mutate {
            path: p,
            expected_version,
            ..
        }
        | NativeTaskCommand::Occurrence {
            path: p,
            expected_version,
            ..
        } => {
            path(p)?;
            version(expected_version)?;
        }
        NativeTaskCommand::Submit { request } => {
            id(&request.request_id)?;
            path(&request.path)?;
            version(&request.expected_version)?;
        }
        NativeTaskCommand::Inspect { request_id } | NativeTaskCommand::Retry { request_id } => {
            id(request_id)?
        }
        NativeTaskCommand::Configure { configuration } => {
            if configuration.daily.folder.len() > 4096
                || configuration.daily.date_format.len() > 1024
                || configuration.format.locale.len() > 64
                || configuration.day_parts.afternoon_starts_at.len() > 16
                || configuration.day_parts.evening_starts_at.len() > 16
                || matches!(&configuration.provider,NativeTaskProvider::Unavailable{message} if message.is_empty() || message.len()>2048)
            {
                return Err(invalid());
            }
        }
        NativeTaskCommand::Status {} | NativeTaskCommand::Disable {} => {}
    }
    Ok(())
}
