//! Path-free remote commands; host admission supplies owner, vault and custody.
mod types;
mod values;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
pub use types::*;
pub use values::*;

// Secret-bearing input deliberately has no Debug or Serialize implementation.
#[derive(Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RemoteCredentialInput {
    pub label: String,
    pub host: String,
    pub username: String,
    pub token: String,
    pub provider: RemoteCredentialProvider,
    #[ts(optional)]
    pub replace: Option<String>,
}
#[derive(Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum RepositoryRemotesCommand {
    SaveConnection {
        #[ts(type = "number")]
        expected_revision: u64,
        target: RemoteRepositoryTarget,
        connection: RemoteConnection,
    },
    TestConnection {
        #[ts(type = "number")]
        expected_revision: u64,
        target: RemoteRepositoryTarget,
    },
    Fetch {
        #[ts(type = "number")]
        expected_revision: u64,
        target: RemoteRepositoryTarget,
    },
    ReviewPush {
        #[ts(type = "number")]
        expected_revision: u64,
        target: RemoteRepositoryTarget,
    },
    Push {
        #[ts(type = "number")]
        expected_revision: u64,
        target: RemoteRepositoryTarget,
        review_id: String,
    },
    ObservePush {
        #[ts(type = "number")]
        expected_revision: u64,
        target: RemoteRepositoryTarget,
        request_id: String,
    },
    StopPush {
        #[ts(type = "number")]
        expected_revision: u64,
        target: RemoteRepositoryTarget,
        request_id: String,
    },
    SetAutomaticSync {
        #[ts(type = "number")]
        expected_revision: u64,
        enabled: bool,
    },
    SaveCredential {
        #[ts(type = "number")]
        expected_revision: u64,
        input: RemoteCredentialInput,
    },
    ForgetCredential {
        #[ts(type = "number")]
        expected_revision: u64,
        id: String,
    },
    BeginSignIn {
        #[ts(type = "number")]
        expected_revision: u64,
    },
    CheckSignIn {
        #[ts(type = "number")]
        expected_revision: u64,
    },
    CancelSignIn {
        #[ts(type = "number")]
        expected_revision: u64,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RemoteAuthorization {
    pub code: String,
    pub url: String,
    pub state: RemoteAuthorizationState,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RemoteSettings {
    pub host: RemoteHost,
    pub storage: RemoteStorageState,
    pub ssh_available: bool,
    pub device_sign_in_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub authorization: Option<RemoteAuthorization>,
    pub credentials: Vec<RemoteCredentialSummary>,
    pub targets: Vec<RemoteRepositorySettings>,
    pub automatic_sync: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct RepositoryRemotesSnapshot {
    #[ts(type = "number")]
    pub revision: u64,
    pub capabilities: RemoteCapabilities,
    pub settings: RemoteSettings,
}
