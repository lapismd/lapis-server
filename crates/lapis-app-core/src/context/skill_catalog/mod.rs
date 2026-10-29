//! Application skill policy; no callback or path becomes execution authority.
mod discovery;
mod parser;
mod precedence;
mod registered;
mod registry;
mod types;
pub use discovery::discover_skills;
use lapis_app_protocol::ErrorCode;
pub use parser::{parse_skill, skill_content_version};
pub use registry::SkillRegistry;
pub use types::*;

fn failure(message: impl Into<String>) -> SkillFailure {
    SkillFailure {
        name: "Error".into(),
        message: message.into(),
        path: None,
        code: ErrorCode::InvalidRequest,
    }
}
impl From<crate::services::Error> for SkillFailure {
    fn from(error: crate::services::Error) -> Self {
        Self {
            code: error.code(),
            ..failure(error.to_string())
        }
    }
}
impl From<SkillFailure> for crate::services::Error {
    fn from(error: SkillFailure) -> Self {
        Self::new(error.code, &error.message)
    }
}
impl std::fmt::Display for SkillFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for SkillFailure {}
