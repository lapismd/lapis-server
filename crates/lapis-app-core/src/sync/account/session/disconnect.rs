use super::{AccountSession, AccountSnapshot, AccountVerifier, admit_time, blocked, record::Phase};
use crate::sync::account::{
    DiscoveryCredentialStatus as Credential, HandoffInspection, HandoffStatus as Status,
};
use std::io;
impl AccountSession {
    pub fn disconnect(&mut self, now: i64) -> io::Result<AccountSnapshot> {
        admit_time(now)?;
        let record = self.load()?;
        if matches!(record.phase, Phase::Disconnected {}) {
            return self.snapshot();
        }
        if matches!(record.phase, Phase::Prepared { .. }) {
            return self.abandon_unissued(now);
        }
        if let Phase::SigningOut { handoff } = &record.phase {
            // An earlier mutation may have completed. Never blindly send again.
            let observed = self.endpoint.inspect_handoff(
                handoff.id,
                self.store.client_id(),
                &AccountVerifier::from_protected(handoff.proof.to_string())?,
            )?;
            if terminal(&observed) {
                self.save(Phase::Disconnected {})?;
                return self.snapshot();
            }
        }
        let credential = match &record.phase {
            Phase::Connected { credential, .. } if credential.expires_at > now => {
                Some(self.endpoint.restore_credential(
                    self.endpoint.origin(),
                    credential.id,
                    self.store.client_id(),
                    credential.expires_at,
                    credential.token.to_string(),
                )?)
            }
            _ => None,
        };
        let handoff = match record.phase {
            Phase::AwaitingApproval { handoff }
            | Phase::Redeeming { handoff }
            | Phase::Connected { handoff, .. }
            | Phase::SigningOut { handoff } => handoff,
            _ => return Err(blocked()),
        };
        let verifier = AccountVerifier::from_protected(handoff.proof.to_string())?;
        let id = handoff.id;
        self.save(Phase::SigningOut { handoff })?;
        if let Some(credential) = credential {
            self.endpoint.sign_out(&credential, now)?;
        } else {
            self.endpoint
                .cancel_handoff(id, self.store.client_id(), &verifier)?;
        }
        self.save(Phase::Disconnected {})?;
        self.snapshot()
    }
}
fn terminal(outcome: &HandoffInspection) -> bool {
    matches!(
        (outcome.status, outcome.credential_status),
        (Status::Rejected | Status::Expired, Credential::NotIssued)
            | (Status::Consumed, Credential::Revoked | Credential::Expired)
    )
}
