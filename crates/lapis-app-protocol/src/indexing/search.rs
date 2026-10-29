use crate::database::search::source::SearchChunkingSettings;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SearchProviderContribution {
    #[schemars(length(min = 1, max = 128))]
    pub provider_id: String,
    #[schemars(length(max = 128))]
    pub version: String,
    pub priority: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SearchIndexingSelection {
    #[schemars(length(max = 64))]
    pub providers: Vec<SearchProviderContribution>,
    pub chunking: SearchChunkingSettings,
}
