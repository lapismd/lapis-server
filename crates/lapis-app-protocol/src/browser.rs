use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// Credentials deliberately do not derive Debug.
#[derive(Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SessionInfo {
    pub username: String,
    pub csrf_token: String,
    #[ts(type = "number")]
    pub expires_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct HostInfo {
    pub protocol_major: u32,
    pub build_id: String,
    pub browser_auth: bool,
    pub manage_vaults: bool,
    pub transfer_vaults: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VaultName {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct LogoutReply {
    pub ok: bool,
}
