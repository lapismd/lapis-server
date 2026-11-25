use crate::Schedule;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct JobPermissions {
    pub metadata_read: bool,
    pub frontmatter_read: bool,
    pub notifications: bool,
    pub frontmatter_fields: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct JobDescriptor {
    pub id: String,
    pub title: String,
    pub description: String,
    pub entry: String,
    pub schedule: Schedule,
    pub permissions: JobPermissions,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct JobConfiguration {
    pub fingerprint: String,
    pub schedule: Schedule,
    pub permissions: JobPermissions,
    pub enabled: bool,
    #[serde(default)]
    pub native_notifications: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum JobPhase {
    Paused,
    Ready,
    Running,
    Completed,
    Failed,
    Interrupted,
    Transferred,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct JobRun {
    pub id: String,
    pub occurrence: String,
    pub local_date: String,
    #[ts(type = "number")]
    pub scheduled_at: i64,
    #[ts(type = "number")]
    pub started_at: i64,
    #[ts(type = "number | null")]
    pub finished_at: Option<i64>,
    pub phase: JobPhase,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct JobStatus {
    pub plugin_id: String,
    pub descriptor: JobDescriptor,
    pub fingerprint: String,
    pub configuration: Option<JobConfiguration>,
    pub phase: JobPhase,
    pub runs: Vec<JobRun>,
    #[ts(type = "number | null")]
    pub next_run_at: Option<i64>,
    pub unavailable_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct JobTransfer {
    pub id: String,
    pub from_vault_id: String,
    pub to_vault_id: String,
    pub from_runner: String,
    pub to_runner: String,
    pub plugin_id: String,
    pub job_id: String,
    pub configuration: JobConfiguration,
    #[ts(type = "unknown")]
    pub state: serde_json::Value,
    pub last_occurrence: Option<String>,
    #[ts(type = "number")]
    pub last_scheduled_at: i64,
    pub notification_keys: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum JobCommand {
    List {},
    Configure {
        plugin_id: String,
        job_id: String,
        configuration: JobConfiguration,
    },
    Pause {
        plugin_id: String,
        job_id: String,
    },
    RunNow {
        plugin_id: String,
        job_id: String,
    },
    Resolve {
        plugin_id: String,
        job_id: String,
        run_id: String,
    },
    Fence {
        trusted: bool,
        safe_mode: bool,
    },
    Transfer {
        plugin_id: String,
        job_id: String,
        to_runner: String,
        to_vault_id: String,
    },
    ImportTransfer {
        receipt: Box<JobTransfer>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct JobSnapshot {
    #[ts(type = "number")]
    pub revision: u64,
    pub runner_id: String,
    pub vault_id: String,
    pub native_notifications_available: bool,
    pub trusted: bool,
    pub safe_mode: bool,
    pub jobs: Vec<JobStatus>,
    pub transfer: Option<JobTransfer>,
}
