//! Transport-neutral plugin operations. Descriptors are data, never authority.
use crate::content::{ContentEffect, ContentError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use uuid::Uuid;

pub const OPERATION_PROTOCOL_MAJOR: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OperationArgument {
    pub flag: String,
    pub field: String,
    pub description: String,
    pub required: bool,
    /// Structured values are passed as JSON, never evaluated as code.
    pub json: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OperationHelp {
    pub command: Vec<String>,
    pub summary: String,
    pub markdown: String,
    pub examples: Vec<String>,
    pub keywords: Vec<String>,
    pub arguments: Vec<OperationArgument>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OperationDescriptor {
    pub plugin_id: String,
    pub id: String,
    pub version: u32,
    pub description: String,
    pub permission: String,
    pub effect: ContentEffect,
    #[ts(type = "Record<string, unknown>")]
    pub input_schema: Value,
    #[ts(type = "Record<string, unknown>")]
    pub output_schema: Value,
    pub help: OperationHelp,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OperationCatalog {
    pub protocol_version: u32,
    pub operations: Vec<OperationDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OperationRequest {
    pub protocol_version: u32,
    pub request_id: Uuid,
    pub version: u32,
    #[ts(type = "Record<string, unknown>")]
    pub input: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "status", rename_all = "snake_case")]
pub enum OperationOutcome {
    Ok {
        #[ts(type = "unknown")]
        value: Value,
    },
    Error {
        error: ContentError,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OperationReply {
    pub protocol_version: u32,
    pub request_id: Uuid,
    pub outcome: OperationOutcome,
}

/// A manifest requests a compiled provider contract; it cannot supply code.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OperationContribution {
    pub id: String,
    pub version: u32,
}
