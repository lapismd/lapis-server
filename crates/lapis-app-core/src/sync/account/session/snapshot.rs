use super::record::{Phase, Record};
use serde::Serialize;
use uuid::Uuid;
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AccountPhase {
    Disconnected,
    Prepared,
    Initiating,
    AwaitingApproval,
    Redeeming,
    Connected,
    SigningOut,
}
/// Only this allowlisted public value may cross presentation IPC.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct AccountSnapshot {
    pub client_id: Uuid,
    pub origin: String,
    pub phase: AccountPhase,
    pub connection_id: Option<Uuid>,
    pub approval_url: Option<String>,
    pub expires_at: Option<i64>,
    /// Earliest safe explicit abandonment; absent for issued/uncertain credentials.
    pub abandon_after: Option<i64>,
    pub credential_expires_at: Option<i64>,
}
impl AccountSnapshot {
    pub(super) fn from_record(record: &Record, client_id: Uuid, origin: &str) -> Self {
        let phase = match record.phase {
            Phase::Disconnected {} => AccountPhase::Disconnected,
            Phase::Prepared { .. } => AccountPhase::Prepared,
            Phase::Initiating { .. } => AccountPhase::Initiating,
            Phase::AwaitingApproval { .. } => AccountPhase::AwaitingApproval,
            Phase::Redeeming { .. } => AccountPhase::Redeeming,
            Phase::Connected { .. } => AccountPhase::Connected,
            Phase::SigningOut { .. } => AccountPhase::SigningOut,
        };
        let handoff = record.handoff();
        Self {
            client_id,
            origin: origin.into(),
            phase,
            connection_id: handoff.map(|h| h.id),
            approval_url: handoff
                .filter(|_| phase == AccountPhase::AwaitingApproval)
                .map(|h| format!("{origin}/?sync-connect={}", h.id)),
            expires_at: handoff.map(|h| h.expires_at),
            abandon_after: match &record.phase {
                Phase::Prepared { .. } => Some(0),
                Phase::Initiating { abandon_after, .. } => Some(*abandon_after),
                Phase::AwaitingApproval { handoff } => {
                    Some(handoff.expires_at.saturating_add(60_000))
                }
                _ => None,
            },
            credential_expires_at: match &record.phase {
                Phase::Connected { credential, .. } => Some(credential.expires_at),
                _ => None,
            },
        }
    }
}
