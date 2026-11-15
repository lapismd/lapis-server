use super::*;
use crate::services::{
    Client,
    plugin_operations::{OperationProvider, failure},
};
use lapis_app_protocol::{content::*, operations::*};
use serde_json::Value;

pub struct SkillsProvider {
    bundled: Vec<PackagedSkill>,
}
impl Default for SkillsProvider {
    fn default() -> Self {
        Self {
            bundled: vec![cli_skill()],
        }
    }
}
pub fn cli_skill() -> PackagedSkill {
    PackagedSkill {
        name: "lapis-cli".into(),
        source: "lapis-cli".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        files: BTreeMap::from([
            (
                "SKILL.md".into(),
                include_str!("../../skills/lapis-cli/SKILL.md").into(),
            ),
            (
                "references/commands.md".into(),
                include_str!("../../skills/lapis-cli/references/commands.md").into(),
            ),
            (
                "references/hosts-and-credentials.md".into(),
                include_str!("../../skills/lapis-cli/references/hosts-and-credentials.md").into(),
            ),
            (
                "references/skill-management.md".into(),
                include_str!("../../skills/lapis-cli/references/skill-management.md").into(),
            ),
        ]),
    }
}
impl SkillsProvider {
    /// Only the owning host may supply bundled package resources.
    pub fn with_bundled(mut self, bundles: Vec<PackagedSkill>) -> Self {
        self.bundled.extend(bundles);
        self
    }
}
impl OperationProvider for SkillsProvider {
    fn descriptors(&self) -> Vec<OperationDescriptor> {
        serde_json::from_str(include_str!("../../skills/operations.json"))
            .expect("valid packaged skill descriptors")
    }
    fn available(&self, client: &Client) -> Result<bool, ContentError> {
        client
            .read_text(".lapis/app.json")
            .or_else(|error| {
                if error.code() == lapis_app_protocol::ErrorCode::NotFound {
                    Ok(lapis_app_protocol::TextSnapshot {
                        path: String::new(),
                        text: String::new(),
                        version: String::new(),
                    })
                } else {
                    Err(error)
                }
            })
            .map(|_| true)
            .map_err(|_| failure(ContentErrorCode::Unavailable, "Skill service unavailable"))
    }
    fn invoke(
        &self,
        client: &Client,
        id: &str,
        request: &OperationRequest,
    ) -> Result<Value, ContentError> {
        let results = client
            .manage_packaged_skills(&self.bundled, id, request.input["skill"].as_str())
            .map_err(|_| {
                let mut error = failure(
                    if id == "list" {
                        ContentErrorCode::Unavailable
                    } else {
                        ContentErrorCode::OutcomeUnknown
                    },
                    "Skill operation failed; inspect installation receipts before retrying",
                );
                if id != "list" {
                    error.effect = ContentEffectStatus::Unknown;
                }
                error
            })?;
        serde_json::to_value(results)
            .map_err(|_| failure(ContentErrorCode::Unavailable, "Skill result unavailable"))
    }
}
