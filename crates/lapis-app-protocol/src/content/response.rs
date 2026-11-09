use super::ContentOperationDescriptor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentCapabilities {
    pub api_version: u32,
    pub operations: Vec<ContentOperationDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentVaultSummary {
    pub vault_id: Uuid,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentNoteSummary {
    pub path: String,
    #[ts(type = "number")]
    pub size_bytes: u64,
}

#[derive(Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum ContentValue {
    Capabilities {
        capabilities: ContentCapabilities,
    },
    Vaults {
        items: Vec<ContentVaultSummary>,
        next_cursor: Option<String>,
    },
    Notes {
        items: Vec<ContentNoteSummary>,
        next_cursor: Option<String>,
    },
    Note {
        path: String,
        text: String,
        version: String,
    },
    /// Committed file version, not an assertion of indexed-search freshness.
    Written {
        path: String,
        version: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContentErrorCode {
    InvalidRequest,
    ProtocolMismatch,
    Unauthenticated,
    PermissionDenied,
    NotFound,
    Conflict,
    AlreadyExists,
    Busy,
    Unsupported,
    TooLarge,
    Unavailable,
    OutcomeUnknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ContentEffectStatus {
    NotStarted,
    Committed,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentError {
    pub code: ContentErrorCode,
    pub message: String,
    pub effect: ContentEffectStatus,
}

#[derive(Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "status", rename_all = "snake_case")]
pub enum ContentOutcome {
    Ok { value: ContentValue },
    Error { error: ContentError },
}

#[derive(Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentReply {
    pub api_version: u32,
    pub request_id: Uuid,
    pub outcome: ContentOutcome,
}
