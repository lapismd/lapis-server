//! Owner-control status only. These DTOs are not content operations and confer no authority.
use super::ContentScope;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentAccessStatus {
    pub enabled: bool,
    pub grants: Vec<ContentGrantSummary>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentGrantSummary {
    pub grant_id: Uuid,
    pub label: String,
    pub vault_ids: Vec<Uuid>,
    pub scope: ContentScope,
    pub active: bool,
    /// Rounded up from the native monotonic deadline, never more than 86400.
    pub expires_in_seconds: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentHostStatus {
    pub access: ContentAccessStatus,
    pub available: bool,
    pub persistent: bool,
    pub api_url: Option<String>,
    pub mcp_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentAccessSetting {
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentGrantRequest {
    pub label: String,
    pub vault_ids: Vec<Uuid>,
    pub lifetime_seconds: u32,
}

/// One-time owner response. Deliberately not Debug or Clone.
#[derive(Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CreatedContentGrant {
    pub grant_id: Uuid,
    pub token: String,
}
