use super::invalid;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io;
use uuid::Uuid;
use zeroize::Zeroizing;

/// Host-only proof. Deliberately has neither Debug nor Serialize.
pub struct AccountVerifier(Zeroizing<String>);
impl AccountVerifier {
    pub fn generate() -> io::Result<Self> {
        let mut random = Zeroizing::new([0_u8; 32]);
        getrandom::fill(random.as_mut()).map_err(|_| invalid())?;
        Self::from_protected(URL_SAFE_NO_PAD.encode(random.as_slice()))
    }
    pub fn from_protected(value: String) -> io::Result<Self> {
        let value = Zeroizing::new(value);
        validate_secret(&value)?;
        Ok(Self(value))
    }
    pub fn challenge(&self) -> String {
        URL_SAFE_NO_PAD.encode(Sha256::digest(self.0.as_bytes()))
    }
    /// Only for the host's protected credential adapter; never renderer IPC.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

/// Host-owned discovery bearer, never a renderer/presentation payload.
pub struct DiscoveryCredential {
    pub id: Uuid,
    pub client_id: Uuid,
    pub expires_at: i64,
    token: Zeroizing<String>,
    origin: String,
}
impl DiscoveryCredential {
    pub(super) fn new(
        origin: String,
        id: Uuid,
        client_id: Uuid,
        expires_at: i64,
        token: String,
    ) -> io::Result<Self> {
        let token = Zeroizing::new(token);
        validate_secret(&token)?;
        if id.is_nil() || client_id.is_nil() || expires_at <= 0 {
            return Err(invalid());
        }
        Ok(Self {
            id,
            client_id,
            expires_at,
            token,
            origin,
        })
    }
    /// Only for the host's protected credential adapter; never renderer IPC.
    pub fn expose_secret(&self) -> &str {
        &self.token
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AccountConnection {
    pub id: Uuid,
    pub client_id: Uuid,
    pub client_name: String,
    pub expires_at: i64,
    pub approval_url: String,
}
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DiscoveredVault {
    pub vault_id: Uuid,
    pub name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConnectionWire {
    pub id: Uuid,
    pub client_id: Uuid,
    pub client_name: String,
    pub expires_at: i64,
    pub approval_url: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CredentialWire {
    pub id: Uuid,
    pub client_id: Uuid,
    pub expires_at: i64,
    pub token: String,
    pub scope: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct VaultsWire {
    pub vaults: Vec<DiscoveredVault>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SignOutWire {
    pub ok: bool,
}

pub(super) fn validate_secret(value: &str) -> io::Result<()> {
    let decoded = Zeroizing::new(URL_SAFE_NO_PAD.decode(value).map_err(|_| invalid())?);
    if decoded.len() != 32 || URL_SAFE_NO_PAD.encode(decoded.as_slice()) != value {
        return Err(invalid());
    }
    Ok(())
}
pub(super) fn valid_name(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
pub(super) fn valid_expiry(expires_at: i64, now: i64, lifetime: i64) -> bool {
    now >= 0
        && expires_at > now
        && expires_at <= now.saturating_add(lifetime).saturating_add(60_000)
}
