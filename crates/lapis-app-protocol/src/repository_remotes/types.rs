use super::values::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum RemoteRepositoryTarget {
    Backup,
    Original,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(rename_all = "kebab-case")]
pub enum RemoteAuthentication {
    Anonymous,
    Token,
    Forge,
    SshAgent,
    SshDefault,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RemoteConnection {
    pub name: String,
    pub url: String,
    pub branch: String,
    pub bookmark: String,
    pub authentication: RemoteAuthentication,
    pub credential: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RemoteCapabilities {
    pub targets: Vec<RemoteRepositoryTarget>,
    pub anonymous_https: bool,
    pub https_tokens: bool,
    pub host_ssh_agent: bool,
    pub host_ssh_default: bool,
    pub github_device_sign_in: bool,
    pub forgejo_tokens: bool,
}
/// Public inventory has opaque identities and labels, never stored token bytes.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RemoteCredentialSummary {
    pub id: String,
    pub label: String,
    pub host: String,
    pub username: String,
    pub method: RemoteCredentialMethod,
    pub available: bool,
}
/// Full immutable destination and commit are retained in the native review.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RemotePushReview {
    pub id: String,
    pub commit: String,
    pub summary: String,
    pub outgoing: u32,
    pub remote_commit: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RemoteObservation {
    pub incoming: u32,
    pub outgoing: u32,
    pub fetched_at: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RemotePublication {
    pub id: String,
    pub commit: String,
    pub outcome: RemotePublicationState,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RemoteRepositorySettings {
    pub target: RemoteRepositoryTarget,
    pub connection: Option<RemoteConnection>,
    pub status: RemoteConnectionState,
    pub bookmarks: Vec<String>,
    pub can_fetch: bool,
    pub can_push: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub observation: Option<RemoteObservation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub review: Option<RemotePushReview>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub publication: Option<RemotePublication>,
}
