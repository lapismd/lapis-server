//! Data-only queue controls. Native installation and journal authority stay in Core.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use uuid::Uuid;

pub const QUEUE_REQUEST_LIMIT: usize = 1024 * 1024;
pub const QUEUE_RECORD_LIMIT: usize = 8 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct QueueRecipient {
    pub id: String,
    pub label: String,
    pub token: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct QueueModel {
    pub provider: String,
    pub model: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum QueueAgent {
    Codex,
    Cursor,
    Opencode,
    Local,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum QueueThinking {
    Default,
    Off,
    Low,
    Medium,
    High,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct QueueCaptureTarget {
    pub id: String,
    pub agent: QueueAgent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model: Option<QueueModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub thinking: Option<QueueThinking>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub execution_mode: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct QueueSelection {
    pub recipient: QueueRecipient,
    pub target: QueueCaptureTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub thinking: Option<QueueThinking>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct QueueAccept {
    pub acceptance_id: Uuid,
    pub conversation_id: Uuid,
    pub after_request_id: Uuid,
    pub scope_dir: String,
    pub target_id: String,
    #[ts(type = "unknown")]
    pub capture: Value,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum QueuePhase {
    Pending,
    Started,
    Completed,
    Cancelled,
    Rejected,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct QueueEntry {
    pub input: QueueAccept,
    pub request_id: Uuid,
    pub phase: QueuePhase,
    #[ts(type = "unknown | null")]
    pub receipt: Option<Value>,
    pub receipt_published: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct QueueRecord {
    pub schema_version: u32,
    pub vault_id: Uuid,
    pub conversation_id: Uuid,
    pub scope_dir: String,
    pub head_request_id: Uuid,
    pub paused: bool,
    pub barrier: Option<String>,
    pub entries: Vec<QueueEntry>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum QueueCommand {
    Accept {
        input: Box<QueueAccept>,
    },
    Read {
        conversation_id: Uuid,
    },
    Stop {
        conversation_id: Uuid,
    },
    Cancel {
        conversation_id: Uuid,
        acceptance_id: Uuid,
    },
    Resume {
        conversation_id: Uuid,
        after_request_id: Uuid,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum QueueValue {
    Queue { queue: Option<Box<QueueRecord>> },
}
