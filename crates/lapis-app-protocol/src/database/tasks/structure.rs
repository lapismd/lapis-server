use super::enums::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct TaskStructuralEntry {
    pub source_document_id: String,
    pub source_document_path: String,
    pub target_document_id: String,
    pub target_document_path: String,
    pub kind: TaskStructuralKind,
    pub heading_path: Vec<String>,
    pub heading_level: Option<f64>,
    pub heading_text: Option<String>,
    pub run_id: String,
    pub position_in_run: f64,
    pub source_start: f64,
    pub source_end: f64,
    pub authored_label: String,
    pub authored_href: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct TaskRun {
    pub id: String,
    pub document_id: String,
    pub document_path: String,
    pub heading_path: Vec<String>,
    pub heading_level: Option<f64>,
    pub heading_text: Option<String>,
    pub list_depth: f64,
    pub start_offset: f64,
    pub end_offset: f64,
    pub entry_count: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct TaskStructure {
    pub version: f64,
    pub source_hash: String,
    pub entries: Vec<TaskStructuralEntry>,
    pub runs: Vec<TaskRun>,
    pub diagnostics: Vec<TaskStructureDiagnostic>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct TaskStructureDiagnostic {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source_start: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source_end: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target: Option<String>,
}
