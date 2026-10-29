//! Existing namespaced plugin projection write contracts.
use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

pub mod queries;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum ProjectionFieldType {
    String,
    Number,
    Boolean,
    Date,
    Datetime,
    Path,
    Json,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum ProjectionVisibility {
    Public,
    Private,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum ProjectionCardinality {
    One,
    Many,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ProjectionField {
    pub r#type: ProjectionFieldType,
    #[serde(default)]
    #[ts(as = "Option<bool>", optional)]
    pub indexed: bool,
    #[serde(default)]
    #[ts(as = "Option<bool>", optional)]
    pub sortable: bool,
    #[serde(default)]
    #[ts(as = "Option<bool>", optional)]
    pub unique: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cardinality: Option<ProjectionCardinality>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ProjectionDefinition {
    pub projection_id: String,
    pub owner_plugin_id: String,
    pub schema_version: f64,
    pub config_hash: String,
    pub visibility: ProjectionVisibility,
    pub fields: IndexMap<String, ProjectionField>,
    pub active: bool,
    pub updated_at: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ProjectionRow {
    pub id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ordinal: Option<f64>,
    #[ts(type = "{ [key: string]: unknown }")]
    pub data: Map<String, Value>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ProjectionEdge {
    pub source_row_id: String,
    pub relation: String,
    #[serde(default)]
    #[ts(optional)]
    pub target_projection_id: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub target_row_id: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub target_path: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub target_text: Option<String>,
    pub ordinal: f64,
    #[serde(default)]
    #[ts(type = "{ [key: string]: unknown } | null", optional)]
    pub data: Option<Map<String, Value>>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ProjectionSource {
    pub projection_id: String,
    pub source_path: String,
    pub source_hash: String,
    pub rows: Vec<ProjectionRow>,
    #[serde(default)]
    #[ts(as = "Option<Vec<ProjectionEdge>>", optional)]
    pub edges: Vec<ProjectionEdge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub writer_plugin_id: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ProjectionSourceError {
    pub projection_id: String,
    pub source_path: String,
    pub source_hash: String,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub writer_plugin_id: Option<String>,
}
