use lapis_app_protocol::content::*;

pub(crate) fn failure(code: ContentErrorCode) -> ContentError {
    ContentError {
        code,
        effect: ContentEffectStatus::NotStarted,
        message: message(code).into(),
    }
}
pub(crate) fn message(code: ContentErrorCode) -> &'static str {
    match code {
        ContentErrorCode::InvalidRequest => {
            "Invalid content input; check command flags, target and credential source"
        }
        ContentErrorCode::Unauthenticated => "Content credential is missing, invalid or expired",
        ContentErrorCode::PermissionDenied => "Content access is not permitted",
        ContentErrorCode::NotFound => "Requested content was not found",
        ContentErrorCode::Conflict | ContentErrorCode::AlreadyExists => {
            "Content state conflicts; refresh before retrying"
        }
        ContentErrorCode::Busy => "Content host is busy",
        ContentErrorCode::Unavailable => "Content host or local credential source is unavailable",
        ContentErrorCode::ProtocolMismatch => {
            "Content response does not match the requested API contract"
        }
        ContentErrorCode::TooLarge => "Content exceeds its size limit",
        ContentErrorCode::Unsupported => "Content operation is not supported",
        ContentErrorCode::OutcomeUnknown => "Content outcome is unknown",
    }
}
pub(crate) fn exit_code(code: ContentErrorCode) -> u8 {
    match code {
        ContentErrorCode::InvalidRequest => 2,
        ContentErrorCode::Unauthenticated => 3,
        ContentErrorCode::Conflict | ContentErrorCode::AlreadyExists => 4,
        ContentErrorCode::NotFound => 5,
        ContentErrorCode::PermissionDenied => 6,
        ContentErrorCode::Busy | ContentErrorCode::Unavailable => 7,
        ContentErrorCode::ProtocolMismatch => 8,
        ContentErrorCode::TooLarge => 9,
        ContentErrorCode::Unsupported => 10,
        ContentErrorCode::OutcomeUnknown => 11,
    }
}
