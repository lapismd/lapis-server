//! Passive native synchronization observations and explicit recovery controls.
use crate::repository_restore::RepositoryCheckpoint;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum RepositorySyncCommand {
    Run {
        #[ts(type = "number")]
        expected_revision: u64,
    },
    ResumeApply {
        #[ts(type = "number")]
        expected_revision: u64,
        request_id: String,
    },
    KeepCurrent {
        #[ts(type = "number")]
        expected_revision: u64,
    },
    Inspect {
        #[ts(type = "number")]
        expected_revision: u64,
        candidate: String,
        path: String,
    },
    Resolve {
        #[ts(type = "number")]
        expected_revision: u64,
        candidate: String,
        path: String,
        resolution: RepositorySyncResolution,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum RepositorySyncResolution {
    Term { id: usize },
    Text { text: String, executable: bool },
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum RepositorySyncPhase {
    Ready,
    Fetching,
    Applying,
    Publishing,
    Conflicts,
    Attention,
    Paused,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RepositorySyncProposal {
    pub local: RepositoryCheckpoint,
    pub incoming: RepositoryCheckpoint,
    pub candidate: Option<RepositoryCheckpoint>,
    pub conflicts: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RepositorySyncTerm {
    pub id: usize,
    pub role: String,
    pub kind: String,
    pub text: Option<String>,
    pub executable: Option<bool>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RepositorySyncConflict {
    pub path: String,
    pub terms: Vec<RepositorySyncTerm>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RepositorySyncSnapshot {
    #[ts(type = "number")]
    pub revision: u64,
    pub enabled: bool,
    pub busy: bool,
    pub phase: RepositorySyncPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub proposal: Option<RepositorySyncProposal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub conflict: Option<RepositorySyncConflict>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub last_success: Option<i64>,
    pub recovery: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pending_apply: Option<String>,
}
