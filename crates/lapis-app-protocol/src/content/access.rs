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
    #[serde(default)]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub permissions: Vec<String>,
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
    #[serde(default)]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub operation_permissions: Vec<String>,
    pub access: ContentAccessStatus,
    pub available: bool,
    pub persistent: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<PersonalTokenSummary>>", optional)]
    pub personal_tokens: Vec<PersonalTokenSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<McpConnectionSummary>>", optional)]
    pub mcp_connections: Vec<McpConnectionSummary>,
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
    #[serde(default)]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub permissions: Vec<String>,
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum PersonalTokenLifetime {
    Hour,
    Day,
    Week,
    Days30,
    #[default]
    Days90,
    Year,
    Never,
}

impl PersonalTokenLifetime {
    pub fn seconds(self) -> Option<i64> {
        Some(match self {
            Self::Hour => 3600,
            Self::Day => 86_400,
            Self::Week => 7 * 86_400,
            Self::Days30 => 30 * 86_400,
            Self::Days90 => 90 * 86_400,
            Self::Year => 365 * 86_400,
            Self::Never => return None,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PersonalTokenRequest {
    pub label: String,
    pub vault_ids: Vec<Uuid>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub lifetime: PersonalTokenLifetime,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PersonalTokenSummary {
    pub grant_id: Uuid,
    pub label: String,
    pub vault_ids: Vec<Uuid>,
    pub permissions: Vec<String>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number | null")]
    pub expires_at: Option<i64>,
    #[ts(type = "number | null")]
    pub last_used_at: Option<i64>,
    pub active: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
pub struct McpConnectionSummary {
    pub client_id: String,
    #[serde(flatten)]
    #[ts(flatten)]
    pub grant: PersonalTokenSummary,
}
