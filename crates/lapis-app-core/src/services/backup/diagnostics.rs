//! Only fixed path-free domain guards cross the native string boundary.
use super::failure;
use crate::services::Error;
use lapis_app_protocol::ErrorCode;

pub(super) fn command(message: String) -> Error {
    guard(&message).unwrap_or_else(failure)
}
pub(super) fn restore(message: String) -> Error {
    guard(&message).unwrap_or_else(|| {
        Error::new(
            ErrorCode::Conflict,
            "Repository action did not complete; review current files or retry the same pending restore",
        )
    })
}
fn guard(message: &str) -> Option<Error> {
    // Native JJ's Display adds this fixed prefix. Never strip arbitrary error
    // context or return a suffix without matching the complete known message.
    let message = message.strip_prefix("Vault backup JJ: ").unwrap_or(message);
    let code = match message {
        "The file changed since the restore preview; review it again"
        | "Vault files changed since the restore preview; review it again"
        | "Vault files changed before restoration could start"
        | "Files changed while restoring the vault; later edits were retained" => {
            ErrorCode::Conflict
        }
        "Vault has unresolved editor state"
        | "Vault restore reached its work limit; retry to continue the same restore" => {
            ErrorCode::Busy
        }
        "This file is excluded from vault backup" => ErrorCode::PermissionDenied,
        "Restore of symbolic links is unavailable" => ErrorCode::Unsupported,
        _ => return None,
    };
    Some(Error::new(code, message))
}
