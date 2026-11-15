//! Single native owner for protected account state and bounded blocking transport.
//! Hosts retain this service beyond view lifetime and run it off async executors.
mod connect;
mod disconnect;
mod record;
mod snapshot;
use super::{
    AccountEndpoint, AccountVerifier, DiscoveredVault, HandoffInspection,
    protected::{HostSecretBytes, ProtectedAccountState},
};
use record::{Phase, Record};
pub use snapshot::{AccountPhase, AccountSnapshot};
use std::io;

/// Intentionally has neither Debug nor Serialize. Construction performs no IO;
/// every operation verifies protected scope and state before using the endpoint.
pub struct AccountSession {
    store: ProtectedAccountState,
    endpoint: AccountEndpoint,
}
impl AccountSession {
    pub fn new(store: ProtectedAccountState, endpoint: AccountEndpoint) -> Self {
        Self { store, endpoint }
    }
    pub fn snapshot(&self) -> io::Result<AccountSnapshot> {
        let record = self.load()?;
        Ok(AccountSnapshot::from_record(
            &record,
            self.store.client_id(),
            self.endpoint.origin(),
        ))
    }
    pub fn custody_pending(&self) -> bool {
        self.store.has_pending_write()
    }
    pub fn retry_custody(&mut self) -> io::Result<AccountSnapshot> {
        self.store.retry_pending()?;
        self.snapshot()
    }
    pub fn inspect(&self) -> io::Result<HandoffInspection> {
        let record = self.load()?;
        let handoff = record.handoff().ok_or_else(blocked)?;
        self.endpoint.inspect_handoff(
            handoff.id,
            self.store.client_id(),
            &AccountVerifier::from_protected(handoff.proof.to_string())?,
        )
    }
    pub fn discover(&self, now: i64) -> io::Result<Vec<DiscoveredVault>> {
        let record = self.load()?;
        let Phase::Connected { credential, .. } = record.phase else {
            return Err(blocked());
        };
        let credential = self.endpoint.restore_credential(
            self.endpoint.origin(),
            credential.id,
            self.store.client_id(),
            credential.expires_at,
            credential.token.to_string(),
        )?;
        self.endpoint.discover(&credential, now)
    }
    pub fn close(&mut self) -> io::Result<()> {
        self.store.close()
    }
    pub fn prepare_close(&self) -> io::Result<()> {
        self.store.prepare_close()
    }
    fn load(&self) -> io::Result<Record> {
        if self.store.origin() != self.endpoint.origin() {
            return Err(invalid());
        }
        let record = match self.store.read()? {
            Some(bytes) => serde_json::from_slice(bytes.expose_secret()).map_err(|_| invalid())?,
            None => Record::new(self, Phase::Disconnected {}),
        };
        record.validate(self)?;
        Ok(record)
    }
    fn save(&mut self, phase: Phase) -> io::Result<()> {
        let record = Record::new(self, phase);
        record.validate(self)?;
        let bytes = HostSecretBytes::new(serde_json::to_vec(&record).map_err(|_| invalid())?)?;
        self.store.write(&bytes)
    }
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Protected Sync account session is invalid or foreign",
    )
}
fn blocked() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "Inspect or resolve the existing Sync account operation before continuing",
    )
}
fn admit_time(now: i64) -> io::Result<()> {
    if now < 0 { Err(invalid()) } else { Ok(()) }
}
