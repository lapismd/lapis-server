use super::{
    AccountSession, AccountSnapshot, AccountVerifier, admit_time, blocked, invalid,
    record::{Credential, Handoff, Phase},
};
use crate::sync::account::{DiscoveryCredentialStatus, HandoffStatus, types::valid_name};
use std::io;
use zeroize::Zeroizing;
impl AccountSession {
    pub fn begin(&mut self, name: &str, now: i64) -> io::Result<AccountSnapshot> {
        admit_time(now)?;
        if !valid_name(name, 128) {
            return Err(invalid());
        }
        if !matches!(self.load()?.phase, Phase::Disconnected {}) {
            return Err(blocked());
        }
        let proof = AccountVerifier::generate()?;
        self.save(Phase::Prepared {
            name: name.into(),
            proof: Zeroizing::new(proof.expose_secret().into()),
        })?;
        self.continue_prepared(now)
    }
    /// Only a durably prepared request can begin a first initiation attempt.
    pub fn continue_prepared(&mut self, now: i64) -> io::Result<AccountSnapshot> {
        admit_time(now)?;
        let Phase::Prepared { name, proof } = self.load()?.phase else {
            return Err(blocked());
        };
        let abandon_after = now.checked_add(670_000).ok_or_else(invalid)?;
        let verifier = AccountVerifier::from_protected(proof.to_string())?;
        self.save(Phase::Initiating {
            name: name.clone(),
            proof,
            abandon_after,
        })?;
        let connection = self
            .endpoint
            .initiate(self.store.client_id(), &name, &verifier, now)?;
        self.save(Phase::AwaitingApproval {
            handoff: Handoff {
                id: connection.id,
                expires_at: connection.expires_at,
                proof: Zeroizing::new(verifier.expose_secret().into()),
            },
        })?;
        self.snapshot()
    }
    pub fn redeem(&mut self, now: i64) -> io::Result<AccountSnapshot> {
        admit_time(now)?;
        let Phase::AwaitingApproval { handoff } = self.load()?.phase else {
            return Err(blocked());
        };
        if now >= handoff.expires_at {
            return Err(blocked());
        }
        let verifier = AccountVerifier::from_protected(handoff.proof.to_string())?;
        let observed =
            self.endpoint
                .inspect_handoff(handoff.id, self.store.client_id(), &verifier)?;
        if observed.status != HandoffStatus::Approved
            || observed.credential_status != DiscoveryCredentialStatus::NotIssued
        {
            return Err(blocked());
        }
        let id = handoff.id;
        self.save(Phase::Redeeming { handoff })?;
        let issued = self
            .endpoint
            .redeem(id, self.store.client_id(), &verifier, now)?;
        // Reload the persisted intent, retaining exactly its proof and identity.
        let Phase::Redeeming { handoff } = self.load()?.phase else {
            return Err(invalid());
        };
        self.save(Phase::Connected {
            handoff,
            credential: Credential {
                id: issued.id,
                expires_at: issued.expires_at,
                token: Zeroizing::new(issued.expose_secret().into()),
            },
        })?;
        self.snapshot()
    }
    /// Explicit abandonment is only safe where this coordinator never redeemed.
    pub fn abandon_unissued(&mut self, now: i64) -> io::Result<AccountSnapshot> {
        admit_time(now)?;
        let safe = match self.load()?.phase {
            Phase::Prepared { .. } => true,
            Phase::Initiating { abandon_after, .. } => now >= abandon_after,
            Phase::AwaitingApproval { handoff } => now >= handoff.expires_at.saturating_add(60_000),
            _ => false,
        };
        if !safe {
            return Err(blocked());
        }
        self.save(Phase::Disconnected {})?;
        self.snapshot()
    }
}
