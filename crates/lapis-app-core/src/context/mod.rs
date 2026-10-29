// SPDX-License-Identifier: Apache-2.0
//! Retained application context values; never executable or filesystem authority.
mod attachments;
pub mod bootstrap;
mod initial;
mod projection;
pub mod skill_catalog;
mod skills;
pub use attachments::prepare_attachments;
pub use initial::prepare_initial_context;
pub use projection::project_agent_turn;
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub use skills::{SelectedSkillActivation, SkillManifestEntry, available_skills_manifest};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ContextKind {
    FileContext,
    MemoryRecall,
    ConversationHandoff,
    TranscriptComments,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ContextBlock {
    pub kind: ContextKind,
    pub id: String,
    pub content: String,
    pub metadata: Value,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct PreparedAgentTurn {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct AttachmentSource {
    pub path: String,
    pub sha256: String,
    pub bytes: usize,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct AttachmentContext {
    pub blocks: Vec<ContextBlock>,
    pub sources: Vec<AttachmentSource>,
}
