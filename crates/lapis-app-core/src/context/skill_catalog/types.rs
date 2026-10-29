use lapis_app_protocol::ErrorCode;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SkillSource {
    Folder,
    Vault,
    User,
    Extension,
    Programmatic,
    Bundled,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SkillCommand {
    Model,
    Tool {
        tool: String,
        #[serde(rename = "argMode")]
        arg_mode: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SkillRequirements {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoadedSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: SkillSource,
    pub root: String,
    pub version: String,
    pub user_invocable: bool,
    pub model_invocable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argument_hint: Option<String>,
    pub command: SkillCommand,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirements: Option<SkillRequirements>,
    pub instructions: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SkillParseInput {
    pub path: String,
    pub source: SkillSource,
    pub root: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillContext {
    #[serde(default)]
    pub scope_dir: String,
    pub available_tool_names: Option<Vec<String>>,
    pub enabled_plugin_ids: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SkillDiagnostic {
    pub path: String,
    pub source: SkillSource,
    pub message: String,
    #[serde(rename = "shadowedBy", skip_serializing_if = "Option::is_none")]
    pub shadowed_by: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DiscoveredSkills {
    pub skills: Vec<LoadedSkill>,
    pub diagnostics: Vec<SkillDiagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgrammaticSkill {
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub user_invocable: Option<bool>,
    pub disable_model_invocation: Option<bool>,
    pub argument_hint: Option<String>,
    pub command: Option<SkillCommand>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredSkill {
    pub owner_plugin_id: String,
    pub kind: String,
    pub path: Option<String>,
    pub skill: Option<ProgrammaticSkill>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSources {
    #[serde(default)]
    pub bundled: Vec<LoadedSkill>,
    #[serde(default)]
    pub registered: Vec<RegisteredSkill>,
    #[serde(default)]
    pub extension_files: Vec<(String, Vec<(String, String)>)>,
    #[serde(default)]
    pub extension_roots: Vec<(String, String)>,
    pub user_prefix: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SkillFailure {
    pub name: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip)]
    pub code: ErrorCode,
}
pub type SkillResult<T> = Result<T, SkillFailure>;

pub trait SkillReader {
    fn list(&mut self, prefix: &str) -> crate::services::Result<Vec<String>>;
    fn read(&mut self, path: &str) -> crate::services::Result<String>;
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSnapshotEntry {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub user_invocable: bool,
    pub model_invocable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argument_hint: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSnapshot {
    pub id: String,
    pub created_at: String,
    pub skills: Vec<SkillSnapshotEntry>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SkillActivationRequest {
    pub name: String,
    pub source: String,
    pub arguments: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillActivation {
    pub skill_id: String,
    pub skill_name: String,
    pub version: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
    pub instructions: String,
}
