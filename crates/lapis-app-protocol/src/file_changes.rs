//! Bounded vault-relative filesystem observation; never file bytes or OS paths.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
#[ts(tag = "type", rename_all = "lowercase")]
pub enum FileChange {
    Create { path: String },
    Modify { path: String },
    Delete { path: String },
    Rename { path: String, previous_path: String },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct FileChangeSet {
    #[ts(type = "number")]
    pub revision: u64,
    pub reset: bool,
    pub changes: Vec<FileChange>,
}
