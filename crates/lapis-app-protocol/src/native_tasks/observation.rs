use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum NativeTaskOutcome {
    Completed,
    Pending,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum NativeTaskTrackingKind {
    Boolean,
    Count,
    Duration,
    Number,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum NativeTaskDayPart {
    Anytime,
    Morning,
    Afternoon,
    Evening,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct NativeTaskObservation {
    pub observation_id: String,
    pub task_id: String,
    pub task_path: String,
    pub daily_document_path: String,
    pub occurrence_date: String,
    pub outcome: NativeTaskOutcome,
    pub tracking_kind: NativeTaskTrackingKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub duration_minutes: Option<f64>,
    pub source_start: usize,
    pub source_end: usize,
    pub projection_version: u32,
    pub checked: bool,
    pub title: String,
    pub href: String,
    pub day_part: NativeTaskDayPart,
    pub source: String,
}
