use super::values::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Complete and skip use the journal route, never this one-file route.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
#[ts(tag = "type", rename_all = "lowercase")]
pub enum NativeTaskMutation {
    Checklist {
        position: f64,
        #[serde(rename = "expectedText")]
        #[ts(rename = "expectedText")]
        expected_text: String,
        checked: bool,
    },
    Plan {
        plan: Option<NativeTaskPlan>,
        at: String,
    },
    Start {
        start: NativeTaskStart,
        at: String,
    },
    Deadline {
        deadline: Option<String>,
        at: String,
    },
    Inbox {
        inbox: bool,
        at: String,
    },
    Repeat {
        repeat: Option<Box<NativeTaskRepeat>>,
        at: String,
    },
    Tracking {
        tracking: Option<NativeTaskTracking>,
        at: String,
    },
    Pause {
        at: String,
        #[serde(rename = "localDate")]
        #[ts(rename = "localDate")]
        local_date: Option<String>,
    },
    Resume {
        at: String,
        #[serde(rename = "localDate")]
        #[ts(rename = "localDate")]
        local_date: Option<String>,
    },
    Cancel {
        at: String,
    },
    Reopen {
        at: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
#[ts(tag = "type", rename_all = "lowercase")]
pub enum NativeTaskLifecycleAction {
    Complete {
        at: String,
        #[serde(rename = "localDate")]
        #[ts(rename = "localDate")]
        local_date: String,
        #[serde(rename = "trackedValue")]
        #[ts(rename = "trackedValue")]
        tracked_value: Option<f64>,
    },
    Skip {
        at: String,
        #[serde(rename = "localDate")]
        #[ts(rename = "localDate")]
        local_date: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NativeTaskLifecycleRequest {
    pub request_id: Uuid,
    pub path: String,
    pub expected_version: String,
    pub action: NativeTaskLifecycleAction,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
#[ts(tag = "type", rename_all = "lowercase")]
pub enum NativeTaskCompletion {
    Boolean {},
    Count {
        value: f64,
        unit: String,
    },
    Duration {
        #[serde(rename = "durationMinutes")]
        #[ts(rename = "durationMinutes")]
        duration_minutes: f64,
    },
    Number {
        value: f64,
        unit: String,
    },
}
