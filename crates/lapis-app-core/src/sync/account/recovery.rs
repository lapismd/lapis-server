use super::{AccountEndpoint, AccountVerifier, invalid};
use reqwest::Method;
use serde::Deserialize;
use std::io;
use uuid::Uuid;
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum HandoffStatus {
    Pending,
    Approved,
    Rejected,
    Consumed,
    Expired,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DiscoveryCredentialStatus {
    NotIssued,
    Active,
    Revoked,
    Expired,
    Unavailable,
}
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HandoffInspection {
    pub connection_id: Uuid,
    pub client_id: Uuid,
    pub expires_at: i64,
    pub status: HandoffStatus,
    pub credential_status: DiscoveryCredentialStatus,
}
impl AccountEndpoint {
    /// Proof-bound read only inspection, including a consumed lost response.
    pub fn inspect_handoff(
        &self,
        id: Uuid,
        client: Uuid,
        verifier: &AccountVerifier,
    ) -> io::Result<HandoffInspection> {
        self.handoff_outcome(id, client, verifier, "inspect")
    }
    /// Explicit cancellation/revocation. Inspect an unknown outcome before retry.
    pub fn cancel_handoff(
        &self,
        id: Uuid,
        client: Uuid,
        verifier: &AccountVerifier,
    ) -> io::Result<HandoffInspection> {
        let outcome = self.handoff_outcome(id, client, verifier, "cancel")?;
        if !matches!(
            (outcome.status, outcome.credential_status),
            (
                HandoffStatus::Rejected,
                DiscoveryCredentialStatus::NotIssued
            ) | (HandoffStatus::Consumed, DiscoveryCredentialStatus::Revoked)
        ) {
            return Err(invalid());
        }
        Ok(outcome)
    }
    fn handoff_outcome(
        &self,
        id: Uuid,
        client: Uuid,
        verifier: &AccountVerifier,
        action: &str,
    ) -> io::Result<HandoffInspection> {
        if id.is_nil() || client.is_nil() {
            return Err(invalid());
        }
        let body = format!("{{\"verifier\":\"{}\"}}", verifier.expose_secret());
        let outcome: HandoffInspection = self.json(
            self.request(Method::POST, &format!("/api/v3/sync/account/{id}/{action}"))
                .body(body),
            4096,
        )?;
        if outcome.connection_id != id
            || outcome.client_id != client
            || outcome.expires_at <= 0
            || ((outcome.status != HandoffStatus::Consumed)
                != (outcome.credential_status == DiscoveryCredentialStatus::NotIssued))
        {
            return Err(invalid());
        }
        Ok(outcome)
    }
}
