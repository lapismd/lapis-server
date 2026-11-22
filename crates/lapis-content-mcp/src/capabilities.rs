use lapis_app_protocol::{
    content::{
        ContentEffect, ContentError, ContentOutcome, ContentReply, ContentScope, ContentValue,
    },
    operations::OperationCatalog,
};
use rmcp::{ErrorData, model::Tool};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// MCP capabilities include namespaced tool names in addition to core reads.
#[derive(JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reply {
    api_version: u32,
    request_id: Uuid,
    outcome: Outcome,
}

#[derive(JsonSchema, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum Outcome {
    Ok { value: Value },
    Error { error: ContentError },
}

#[derive(JsonSchema, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Value {
    Capabilities { capabilities: Capabilities },
}

#[derive(JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct Capabilities {
    api_version: u32,
    operations: Vec<Operation>,
}

#[derive(JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    operation: String,
    scope: ContentScope,
    effect: ContentEffect,
}

impl Reply {
    pub(super) fn new(
        reply: ContentReply,
        reads: &[Tool],
        registered: OperationCatalog,
    ) -> Result<Self, ErrorData> {
        let outcome = match reply.outcome {
            ContentOutcome::Ok {
                value: ContentValue::Capabilities { capabilities },
            } => {
                let operations = reads
                    .iter()
                    .map(|tool| Operation::new(tool.name.to_string(), ContentEffect::ReadOnly))
                    .chain(registered.operations.into_iter().map(|descriptor| {
                        Operation::new(super::operations::tool_name(&descriptor), descriptor.effect)
                    }))
                    .collect();
                Outcome::Ok {
                    value: Value::Capabilities {
                        capabilities: Capabilities {
                            api_version: capabilities.api_version,
                            operations,
                        },
                    },
                }
            }
            ContentOutcome::Error { error } => Outcome::Error { error },
            _ => {
                return Err(ErrorData::internal_error(
                    "Invalid capabilities reply",
                    None,
                ));
            }
        };
        Ok(Self {
            api_version: reply.api_version,
            request_id: reply.request_id,
            outcome,
        })
    }
}

impl Operation {
    fn new(operation: String, effect: ContentEffect) -> Self {
        Self {
            operation,
            scope: match effect {
                ContentEffect::ReadOnly => ContentScope::Read,
                ContentEffect::Write => ContentScope::Write,
            },
            effect,
        }
    }
}
