use lapis_app_protocol::content::*;
use ts_rs::{Config, TS};

pub fn declarations(config: &Config) -> Vec<String> {
    vec![
        ContentOperation::decl(config),
        ContentScope::decl(config),
        ContentEffect::decl(config),
        ContentOperationDescriptor::decl(config),
        ContentPageQuery::decl(config),
        ContentCommand::decl(config),
        ContentRequest::decl(config),
        ContentCapabilities::decl(config),
        ContentVaultSummary::decl(config),
        ContentNoteSummary::decl(config),
        ContentValue::decl(config),
        ContentErrorCode::decl(config),
        ContentEffectStatus::decl(config),
        ContentError::decl(config),
        ContentOutcome::decl(config),
        ContentReply::decl(config),
        ContentAccessStatus::decl(config),
        ContentGrantSummary::decl(config),
    ]
}

pub fn constants() -> String {
    format!(
        "export const CONTENT_API_MAJOR = {CONTENT_API_MAJOR} as const;\n\
         export const MAX_CONTENT_PAGE_SIZE = {MAX_CONTENT_PAGE_SIZE} as const;\n\
         export const MAX_CONTENT_TEXT_BYTES = {MAX_CONTENT_TEXT_BYTES} as const;\n\
         export const MAX_CONTENT_PATH_BYTES = {MAX_CONTENT_PATH_BYTES} as const;\n\
         export const MAX_CONTENT_CURSOR_BYTES = {MAX_CONTENT_CURSOR_BYTES} as const;\n\
         export const MAX_CONTENT_VERSION_BYTES = {MAX_CONTENT_VERSION_BYTES} as const;"
    )
}
