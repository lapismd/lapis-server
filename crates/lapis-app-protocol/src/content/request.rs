use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentRequest {
    pub api_version: u32,
    /// Correlation identity; for a mutation this also identifies its durable receipt.
    /// Receipt storage and duplicate suppression belong to the executing service.
    pub request_id: Uuid,
    pub command: ContentCommand,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentPageQuery {
    pub limit: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cursor: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "operation", rename_all = "snake_case")]
pub enum ContentCommand {
    Capabilities {},
    ListVaults {
        page: ContentPageQuery,
    },
    ListNotes {
        vault_id: Uuid,
        /// Omitted means the vault root. Listings are immediate children only.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        directory: Option<String>,
        page: ContentPageQuery,
    },
    ReadNote {
        vault_id: Uuid,
        path: String,
    },
    /// Absent-only publication: an existing destination is never overwritten.
    CreateNote {
        vault_id: Uuid,
        path: String,
        text: String,
    },
    ReplaceNote {
        vault_id: Uuid,
        path: String,
        text: String,
        /// Strong opaque version returned by a read; never a wildcard or timestamp.
        expected_version: String,
    },
}
