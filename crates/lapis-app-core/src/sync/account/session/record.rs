use super::{AccountSession, invalid};
use crate::sync::account::{AccountVerifier, types::valid_name};
use serde::{Deserialize, Serialize};
use std::io;
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    version: u32,
    origin: String,
    client: Uuid,
    pub phase: Phase,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Phase {
    Disconnected {},
    Prepared {
        name: String,
        proof: Zeroizing<String>,
    },
    Initiating {
        name: String,
        proof: Zeroizing<String>,
        abandon_after: i64,
    },
    AwaitingApproval {
        handoff: Handoff,
    },
    Redeeming {
        handoff: Handoff,
    },
    Connected {
        handoff: Handoff,
        credential: Credential,
    },
    SigningOut {
        handoff: Handoff,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Handoff {
    pub id: Uuid,
    pub expires_at: i64,
    pub proof: Zeroizing<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Credential {
    pub id: Uuid,
    pub expires_at: i64,
    pub token: Zeroizing<String>,
}
impl Record {
    pub fn new(session: &AccountSession, phase: Phase) -> Self {
        Self {
            version: 1,
            origin: session.endpoint.origin().into(),
            client: session.store.client_id(),
            phase,
        }
    }
    pub fn validate(&self, session: &AccountSession) -> io::Result<()> {
        if self.version != 1
            || self.origin != session.endpoint.origin()
            || self.client != session.store.client_id()
        {
            return Err(invalid());
        }
        match &self.phase {
            Phase::Prepared { name, proof } | Phase::Initiating { name, proof, .. } => {
                if !valid_name(name, 128) {
                    return Err(invalid());
                }
                AccountVerifier::from_protected(proof.to_string())?;
            }
            Phase::Connected { credential, .. } => {
                session.endpoint.restore_credential(
                    &self.origin,
                    credential.id,
                    self.client,
                    credential.expires_at,
                    credential.token.to_string(),
                )?;
            }
            _ => {}
        }
        if let Phase::Initiating { abandon_after, .. } = self.phase
            && abandon_after <= 0
        {
            return Err(invalid());
        }
        if let Some(handoff) = self.handoff() {
            if handoff.id.is_nil() || handoff.expires_at <= 0 {
                return Err(invalid());
            }
            AccountVerifier::from_protected(handoff.proof.to_string())?;
        }
        Ok(())
    }
    pub fn handoff(&self) -> Option<&Handoff> {
        match &self.phase {
            Phase::AwaitingApproval { handoff }
            | Phase::Redeeming { handoff }
            | Phase::Connected { handoff, .. }
            | Phase::SigningOut { handoff } => Some(handoff),
            _ => None,
        }
    }
}
