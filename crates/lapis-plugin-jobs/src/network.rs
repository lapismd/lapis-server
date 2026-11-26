use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Exact endpoint authority; never an ambient fetch grant.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct JobNetworkGrant {
    pub origin: String,
    pub methods: Vec<String>,
    pub allow_private_network: bool,
    pub credentials: Vec<JobCredentialBinding>,
}

/// An opaque native slot. Only the native request implementation reads its value.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct JobCredentialBinding {
    pub id: String,
    pub header: String,
    pub prefix: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct JobCredentialStatus {
    pub id: String,
    pub configured: bool,
    pub unavailable_reason: Option<String>,
}
