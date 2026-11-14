use lapis_app_protocol::{content::ContentEffect, operations::OperationCatalog};
use rmcp::{
    ErrorData,
    model::{Tool, ToolAnnotations},
};
use serde_json::json;
use std::sync::Arc;

pub fn tools(catalog: OperationCatalog) -> Result<Vec<Tool>, ErrorData> {
    catalog.operations.into_iter().map(|descriptor| {
        let mut schema = descriptor.input_schema;
        schema["properties"]["vault_id"] = json!({
            "type": "string", "format": "uuid",
            "description": "Explicit admitted vault identity"
        });
        schema["properties"]["invocation_id"] = json!({
            "type": "string", "format": "uuid",
            "description": "Caller-chosen stable request UUID; retain for inspection before retrying a write"
        });
        if !schema["required"].is_array() {
            schema["required"] = json!([]);
        }
        schema["required"].as_array_mut().unwrap().push(json!("vault_id"));
        if descriptor.effect == ContentEffect::Write {
            schema["required"].as_array_mut().unwrap().push(json!("invocation_id"));
        }
        let mut tool = Tool::new(
            format!("{}_{}", descriptor.plugin_id, descriptor.id),
            descriptor.description,
            schema.as_object().cloned().ok_or_else(|| {
                ErrorData::internal_error("Invalid operation schema", None)
            })?,
        );
        // MCP returns the transport envelope, including typed failures.
        let output = json!({
            "type": "object",
            "properties": {
                "protocol_version": {"type": "integer"},
                "request_id": {"type": "string"},
                "outcome": {"type": "object"}
            },
            "required": ["protocol_version", "request_id", "outcome"]
        });
        tool.output_schema = Some(Arc::new(output.as_object().unwrap().clone()));
        tool.annotations = Some(
            ToolAnnotations::new()
                .read_only(descriptor.effect == ContentEffect::ReadOnly)
                .destructive(descriptor.effect == ContentEffect::Write)
                .idempotent(descriptor.effect == ContentEffect::ReadOnly)
                .open_world(false),
        );
        Ok(tool)
    }).collect()
}
