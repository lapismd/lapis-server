use lapis_app_protocol::content::{ContentCommand, ContentOperation, ContentReply};
use rmcp::{
    ErrorData,
    model::{JsonObject, Tool, ToolAnnotations},
};
use schemars::{JsonSchema, generate::SchemaSettings};
use serde_json::Value;
use std::{io, sync::Arc};

const READS: &[(&str, ContentOperation, &str)] = &[
    (
        "capabilities",
        ContentOperation::Capabilities,
        "List currently supported content operations.",
    ),
    (
        "list_vaults",
        ContentOperation::ListVaults,
        "List vaults admitted by this credential; use the returned cursor for another page.",
    ),
    (
        "list_notes",
        ContentOperation::ListNotes,
        "List immediate Markdown children of an admitted vault directory.",
    ),
    (
        "read_note",
        ContentOperation::ReadNote,
        "Read saved UTF-8 Markdown and its strong version from an admitted vault.",
    ),
];

pub(crate) struct Catalog {
    pub tools: Vec<Tool>,
}
impl Catalog {
    pub fn new() -> io::Result<Self> {
        let command = schema::<ContentCommand>();
        let variants = command["oneOf"].as_array().ok_or_else(invalid_schema)?;
        let output = Arc::new(
            schema::<ContentReply>()
                .as_object()
                .ok_or_else(invalid_schema)?
                .clone(),
        );
        let mut tools = Vec::new();
        for &(name, operation, description) in READS {
            let operation = serde_json::to_value(operation).map_err(|_| invalid_schema())?;
            let mut input = variants
                .iter()
                .find(|v| v["properties"]["operation"]["const"] == operation)
                .and_then(Value::as_object)
                .ok_or_else(invalid_schema)?
                .clone();
            input
                .get_mut("properties")
                .and_then(Value::as_object_mut)
                .ok_or_else(invalid_schema)?
                .remove("operation");
            input
                .get_mut("required")
                .and_then(Value::as_array_mut)
                .ok_or_else(invalid_schema)?
                .retain(|v| v != "operation");
            let mut tool = Tool::new(name, description, input);
            tool.output_schema = Some(output.clone());
            tool.annotations = Some(
                ToolAnnotations::new()
                    .read_only(true)
                    .destructive(false)
                    .idempotent(true)
                    .open_world(false),
            );
            tools.push(tool);
        }
        Ok(Self { tools })
    }
    pub fn command(
        &self,
        name: &str,
        arguments: Option<JsonObject>,
    ) -> Result<ContentCommand, ErrorData> {
        let (_, operation, _) = READS
            .iter()
            .find(|(id, _, _)| *id == name)
            .ok_or_else(|| ErrorData::invalid_params("Unknown content tool", None))?;
        let mut input = arguments.unwrap_or_default();
        if input.contains_key("operation") {
            return Err(ErrorData::invalid_params(
                "Tool arguments cannot select an operation",
                None,
            ));
        }
        input.insert(
            "operation".into(),
            serde_json::to_value(operation).expect("enum serializes"),
        );
        serde_json::from_value(Value::Object(input))
            .map_err(|_| ErrorData::invalid_params("Invalid content tool arguments", None))
    }
}
fn schema<T: JsonSchema>() -> Value {
    let settings =
        SchemaSettings::draft2020_12().with(|settings| settings.inline_subschemas = true);
    serde_json::to_value(settings.into_generator().into_root_schema_for::<T>())
        .expect("schema serializes")
}
fn invalid_schema() -> io::Error {
    io::Error::other("Content tool schema does not match the Protocol command catalog")
}
