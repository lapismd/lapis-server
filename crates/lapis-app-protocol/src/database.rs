use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use ts_rs::TS;

pub mod call;
pub mod changes;
pub mod history;
pub mod indexed;
pub mod memory;
pub mod projections;
pub mod search;
pub mod state;
pub mod tasks;

// A missing JS value means delete/no check; explicit JSON null remains a value.
fn present<'de, D: Deserializer<'de>>(input: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(input).map(Some)
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MetaCheck {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub exists: Option<bool>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "unknown", optional)]
    pub value: Option<Value>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MetaMutation {
    pub key: String,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "unknown", optional)]
    pub value: Option<Value>,
    #[serde(default)]
    #[ts(as = "Option<bool>", optional)]
    pub delete: bool,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MetaTransaction {
    #[serde(default)]
    #[ts(as = "Option<Vec<MetaCheck>>", optional)]
    pub checks: Vec<MetaCheck>,
    pub mutations: Vec<MetaMutation>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MetaScan {
    pub prefix: String,
    #[serde(default)]
    #[ts(optional)]
    pub after: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub limit: Option<usize>,
}
#[derive(Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MetaEntry {
    pub key: String,
    #[ts(type = "unknown")]
    pub value: Value,
}
#[derive(Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MetaPage {
    pub entries: Vec<MetaEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub next_cursor: Option<String>,
}
