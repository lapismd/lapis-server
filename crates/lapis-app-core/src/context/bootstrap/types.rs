use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapInput {
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub scope_dir: String,
    pub launch_note_path: Option<String>,
    pub workspace_label: Option<String>,
    #[serde(default)]
    pub tools: Vec<BootstrapTool>,
    #[serde(default)]
    pub skills: Vec<BootstrapSkill>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BootstrapTool {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BootstrapSkill {
    pub name: String,
    pub description: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct FolderInstruction {
    pub path: String,
    pub text: String,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub omitted: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BootstrapApplication {
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapConversation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub scope_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub launch_note_path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BootstrapWorkspace {
    pub label: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentBootstrap {
    pub application: BootstrapApplication,
    pub conversation: BootstrapConversation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<BootstrapWorkspace>,
    pub tools: Vec<BootstrapTool>,
    pub skills: Vec<BootstrapSkill>,
    pub folder_instructions: Vec<FolderInstruction>,
    pub instructions: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct PreparedBootstrap {
    pub bootstrap: AgentBootstrap,
    pub text: String,
}
