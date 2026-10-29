use super::{NativeTaskConfiguration, NativeTaskLifecycleRequest};
use crate::TextSnapshot;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum NativeTaskDisposition {
    Retryable,
    Divergent,
    Completed,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NativeTaskKnownFailure {
    pub code: String,
    pub message: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NativeTaskOperation {
    pub request: NativeTaskLifecycleRequest,
    pub disposition: NativeTaskDisposition,
    pub original: TextSnapshot,
    pub task_replacement: String,
    pub daily: Option<TextSnapshot>,
    pub daily_replacement: Option<String>,
    pub last_failure: Option<NativeTaskKnownFailure>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NativeTaskFailure {
    pub name: String,
    pub code: Option<String>,
    pub message: String,
    pub parts: Option<Vec<String>>,
    pub request_id: Option<Uuid>,
    pub uncertain: bool,
    pub operation: Option<Box<NativeTaskOperation>>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NativeTaskTrackedFailure {
    pub path: String,
    pub message: String,
    pub daily_path: Option<String>,
    pub daily_version: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NativeTaskTrackedStatus {
    pub date: String,
    pub attempted: usize,
    pub materialized: usize,
    pub changed: usize,
    pub failures: Vec<NativeTaskTrackedFailure>,
    pub omitted_failures: usize,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NativeTasksStatus {
    pub enabled: bool,
    pub configuration: Option<NativeTaskConfiguration>,
    pub tracked: Option<NativeTaskTrackedStatus>,
}
