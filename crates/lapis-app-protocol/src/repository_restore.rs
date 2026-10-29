use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum RepositoryRestoreCommand {
    Preview {
        checkpoint: String,
        path: String,
    },
    Apply {
        request: RepositoryFileRestoreRequest,
    },
    PreviewVault {
        checkpoint: String,
    },
    ApplyVault {
        request: RepositoryVaultRestoreRequest,
    },
    KeepCurrent {},
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RepositoryFileRestoreRequest {
    pub request_id: String,
    pub checkpoint: String,
    pub path: String,
    pub expected: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RepositoryVaultRestoreRequest {
    pub request_id: String,
    pub checkpoint: String,
    pub expected: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RepositoryFileRestorePreview {
    pub checkpoint: String,
    pub path: String,
    pub expected: String,
    pub bytes: usize,
    pub deleted: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RepositoryVaultRestorePreview {
    pub checkpoint: String,
    pub expected: String,
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RepositoryCheckpoint {
    pub operation: String,
    pub commit: String,
    pub change_id: String,
    pub description: String,
    #[ts(type = "number")]
    pub timestamp_millis: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RepositoryCapture {
    pub changed: bool,
    pub checkpoint: Option<RepositoryCheckpoint>,
    pub operation: String,
    pub working_copy: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(untagged)]
#[ts(untagged)]
pub enum RepositoryRestoreValue {
    File(RepositoryFileRestorePreview),
    Vault(RepositoryVaultRestorePreview),
    Captured(RepositoryCapture),
}
