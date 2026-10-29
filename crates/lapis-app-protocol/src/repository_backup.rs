use crate::{ExpectedVersion, repository_restore::RepositoryCheckpoint};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum RepositoryBackupCommand {
    Status {},
    Capture {},
    SetEnabled {
        enabled: bool,
        expected: ExpectedVersion,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum RepositoryBackupPhase {
    Saving,
    Ready,
    Failed,
    Closed,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RepositoryBackupStatus {
    pub phase: RepositoryBackupPhase,
    pub checkpoint: Option<RepositoryCheckpoint>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RepositoryBackupSnapshot {
    pub enabled: bool,
    pub configuration: ExpectedVersion,
    pub status: RepositoryBackupStatus,
}
