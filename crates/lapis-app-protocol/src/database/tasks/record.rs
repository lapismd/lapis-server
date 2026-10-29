use super::{enums::*, recurrence::*, structure::*};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct TaskRecord {
    pub document_path: String,
    pub document_id: String,
    pub kind: TaskKind,
    pub title: String,
    pub status: TaskStatus,
    pub inbox: bool,
    pub start_kind: TaskStartKind,
    pub checklist_total: f64,
    pub checklist_completed: f64,
    pub comment_count: f64,
    pub projection_version: f64,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub priority: Option<Option<TaskPriority>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub start_date: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub plan_date: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub plan_kind: Option<Option<TaskPlanKind>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub plan_time: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub duration_minutes: Option<Option<f64>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub deadline: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub completed_at: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub repeat: Option<Option<TaskRepeat>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub tracking: Option<Option<TaskTracking>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub effective_occurrence_date: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub effective_occurrence_state: Option<Option<TaskOccurrenceState>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub effective_for_date: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub repeat_strategy: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub repeat_frequency: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub repeat_interval: Option<Option<f64>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub repeat_anchor: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "super::nullable",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub structure: Option<Option<TaskStructure>>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
}
