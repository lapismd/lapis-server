use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum ChangeDomain {
    Metadata,
    Search,
    History,
    Notification,
    Notebook,
    Task,
    Projection,
    Memory,
    Meta,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ChangedPath {
    pub old_path: String,
    pub new_path: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ChangeSet {
    #[ts(type = "number")]
    pub revision: u64,
    pub domains: Vec<ChangeDomain>,
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub renamed: Option<Vec<ChangedPath>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reset: Option<bool>,
    #[ts(type = "number")]
    pub committed_at: i64,
}
