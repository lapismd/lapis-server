use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TurnOptions {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub mode: String,
    #[ts(type = "number")]
    pub max_duration_ms: u64,
}
impl Default for TurnOptions {
    fn default() -> Self {
        Self {
            model: None,
            effort: None,
            mode: "ask".into(),
            max_duration_ms: 60_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TurnContinuation {
    pub conversation_id: Uuid,
    pub after_request_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TurnInput {
    pub request_id: Uuid,
    pub target_id: String,
    pub scope_dir: String,
    pub prompt: String,
    pub context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub continuation: Option<TurnContinuation>,
    #[serde(default)]
    pub options: TurnOptions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum TurnPhase {
    Accepted,
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TurnRecord {
    pub schema_version: u32,
    pub vault_id: Uuid,
    pub conversation_id: Uuid,
    pub input: TurnInput,
    pub phase: TurnPhase,
    #[ts(type = "number")]
    pub accepted_at_ms: u64,
    #[ts(type = "number")]
    pub updated_at_ms: u64,
    pub output: Option<String>,
    pub failure_code: Option<String>,
}
/// Metadata-only native query; prompts, context and response bodies stay on read.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TurnSummary {
    pub request_id: Uuid,
    pub conversation_id: Uuid,
    pub target_id: String,
    pub scope_dir: String,
    pub phase: TurnPhase,
    #[ts(type = "number")]
    pub accepted_at_ms: u64,
    #[ts(type = "number")]
    pub updated_at_ms: u64,
    pub failure_code: Option<String>,
}
impl From<TurnRecord> for TurnSummary {
    fn from(record: TurnRecord) -> Self {
        Self {
            request_id: record.input.request_id,
            conversation_id: record.conversation_id,
            target_id: record.input.target_id,
            scope_dir: record.input.scope_dir,
            phase: record.phase,
            accepted_at_ms: record.accepted_at_ms,
            updated_at_ms: record.updated_at_ms,
            failure_code: record.failure_code,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum TurnCommand {
    Targets {},
    SetEnabled { enabled: bool },
    Submit { input: Box<TurnInput> },
    Read { request_id: Uuid },
    List {},
    Cancel { request_id: Uuid },
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum TurnValue {
    Targets { targets: Vec<TurnTarget> },
    Availability { enabled: bool },
    Record { record: Option<Box<TurnRecord>> },
    Records { records: Vec<TurnSummary> },
    CancelRequested { accepted: bool },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TurnTarget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub queue_selection: Option<crate::queues::QueueSelection>,
    pub id: String,
    pub label: String,
    pub default_mode: String,
}
