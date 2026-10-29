//! Bounded controls for compiled processors; no renderer callbacks or host paths.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
pub mod search;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProjectionContribution {
    #[schemars(length(min = 1, max = 128))]
    pub projection_id: String,
    #[schemars(length(min = 1, max = 128))]
    pub owner_plugin_id: String,
    pub schema_version: f64,
    // Native cache identities are separate; nonempty client hashes need a client.
    #[schemars(length(max = 1024))]
    pub configuration_hash: String,
    #[serde(default)]
    #[schemars(length(max = 1024))]
    pub path_prefix: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum IndexingCommand {
    SelectMetadata {},
    SelectSearch {
        selection: search::SearchIndexingSelection,
    },
    DisableSearch {},
    RegisterProcessor {
        #[schemars(length(min = 1, max = 128))]
        owner_plugin_id: String,
        #[schemars(length(min = 1, max = 32))]
        extension: String,
    },
    RegisterProjection {
        contribution: ProjectionContribution,
    },
    DisableProjection {
        #[schemars(length(min = 1, max = 128))]
        projection_id: String,
    },
    Refresh {
        rebuild: bool,
    },
    Status {},
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum IndexingPhase {
    #[default]
    Idle,
    Discovering,
    Metadata,
    Tasks,
    Docs,
    Search,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct IndexingStatus {
    #[ts(type = "number")]
    pub passes: u64,
    pub indexed: usize,
    pub deleted: usize,
    pub pending: usize,
    pub error: Option<String>,
    pub active: bool,
    pub processed: usize,
    pub total: usize,
    pub phase: IndexingPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub completed_at: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ContributionPlacement {
    Native,
    ClientDependent,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContributionStatus {
    pub projection_id: String,
    pub placement: ContributionPlacement,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct IndexingValue {
    pub status: Option<IndexingStatus>,
    pub contribution: Option<ContributionStatus>,
}
