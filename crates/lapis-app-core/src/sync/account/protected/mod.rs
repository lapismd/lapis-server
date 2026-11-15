//! Native-only protected state. No renderer serialization or persistence fallback.
mod codec;
mod pending;
mod scope;
use super::AccountEndpoint;
use crate::vault::VaultRoot;
use scope::Owner;
use std::{io, sync::Arc};
use uuid::Uuid;
use zeroize::Zeroizing;

pub const MAX_PROTECTED_RECORD_BYTES: usize = 32 * 1024;
pub const MAX_ACCOUNT_PAYLOAD_BYTES: usize = 16 * 1024;

/// Secret-bearing host bytes deliberately implement neither Debug nor Serialize.
pub struct HostSecretBytes(Zeroizing<Vec<u8>>);
impl HostSecretBytes {
    pub fn new(bytes: Vec<u8>) -> io::Result<Self> {
        let bytes = Zeroizing::new(bytes);
        if bytes.is_empty() || bytes.len() > MAX_PROTECTED_RECORD_BYTES {
            return Err(corrupt());
        }
        Ok(Self(bytes))
    }
    /// Native custody and orchestration only; never expose through renderer IPC.
    pub fn expose_secret(&self) -> &[u8] {
        &self.0
    }
}

/// An explicitly selected OS custody adapter. Read returns committed protected
/// bytes, not a staged candidate. Errors must never be converted into absence.
/// Calls are blocking and belong off renderer/async executor threads.
pub trait HostAccountCustody: Send + Sync {
    fn supported(&self) -> bool;
    fn read(&self, key: &str) -> io::Result<Option<HostSecretBytes>>;
    fn write(&self, key: &str, value: &HostSecretBytes) -> io::Result<()>;
}

/// One native owner per captured scope. Hosts must retain this owner if close
/// refuses an uncertain protected write; observers never own its lifetime.
pub struct ProtectedAccountState {
    owner: Owner,
    custody: Arc<dyn HostAccountCustody>,
    pending: Option<pending::Write>,
}
impl ProtectedAccountState {
    pub fn open(
        profile: &VaultRoot,
        vault: &VaultRoot,
        profile_id: Uuid,
        vault_id: Uuid,
        endpoint: &AccountEndpoint,
        custody: Arc<dyn HostAccountCustody>,
    ) -> io::Result<Self> {
        if !custody.supported() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Selected Sync account custody is unavailable",
            ));
        }
        let mut owner = Owner::open(profile, vault, profile_id, vault_id, endpoint.origin())?;
        // Reserve the public client/binding before the first fallible custody
        // lookup. A locked provider must not strand a fresh ownership witness.
        // An established lock without metadata still requires recovery below.
        if owner.fresh_lock && owner.marker_bytes.is_none() {
            owner.commit_marker(&owner.marker.clone())?;
        }
        let raw = custody.read(&owner.key).map_err(custody_error)?;
        if let Some(raw) = &raw {
            let envelope = codec::decode(raw)?;
            owner.validate_scope(&envelope.scope)?;
            if owner.marker_bytes.is_some() && envelope.scope != owner.marker.scope {
                return Err(corrupt());
            }
            if owner.marker_bytes.is_none() {
                owner.marker.scope = envelope.scope;
            }
            let mut marker = owner.marker.clone();
            marker.protected = true;
            owner.commit_marker(&marker)?;
        } else if owner.marker_bytes.is_none() {
            // A retained lock identifies an established or interrupted scope.
            // Losing both metadata and custody must not mint another identity.
            if !owner.fresh_lock {
                return Err(corrupt());
            }
            owner.commit_marker(&owner.marker.clone())?;
        }
        owner.verify_marker()?;
        Ok(Self {
            owner,
            custody,
            pending: None,
        })
    }
    pub fn origin(&self) -> &str {
        &self.owner.marker.scope.origin
    }
    pub fn client_id(&self) -> Uuid {
        self.owner.marker.scope.client
    }
    pub fn has_pending_write(&self) -> bool {
        self.pending.is_some()
    }
    pub fn read(&self) -> io::Result<Option<HostSecretBytes>> {
        self.require_settled()?;
        self.owner.verify_marker()?;
        let raw = self.current_raw()?;
        self.owner.verify_marker()?;
        raw.as_ref()
            .map(|raw| codec::payload(raw, &self.owner.marker.scope))
            .transpose()
    }
    pub fn write(&mut self, payload: &HostSecretBytes) -> io::Result<()> {
        self.require_settled()?;
        self.owner.verify_marker()?;
        if payload.expose_secret().len() > MAX_ACCOUNT_PAYLOAD_BYTES {
            return Err(corrupt());
        }
        let previous = self.current_raw()?;
        let desired = codec::encode(&self.owner.marker.scope, payload)?;
        let mut marker = self.owner.marker.clone();
        marker.protected = true;
        self.pending = Some(pending::Write {
            previous: previous.as_ref().map(pending::digest),
            desired,
            marker,
        });
        self.flush_pending()
    }
    /// Inspect first. A confirmed old value permits retrying identical bytes;
    /// a confirmed new value finishes without another custody mutation.
    pub fn retry_pending(&mut self) -> io::Result<()> {
        if self.pending.is_none() {
            return Ok(());
        }
        self.flush_pending()
    }
    pub fn close(&mut self) -> io::Result<()> {
        self.require_settled()?;
        self.owner.close()
    }
    /// Non-mutating native leave preflight; custody ownership remains retained.
    pub fn prepare_close(&self) -> io::Result<()> {
        self.require_settled()?;
        self.owner.verify_marker()
    }
    fn current_raw(&self) -> io::Result<Option<HostSecretBytes>> {
        let raw = self.custody.read(&self.owner.key).map_err(custody_error)?;
        if let Some(raw) = &raw {
            codec::payload(raw, &self.owner.marker.scope)?;
        } else if self.owner.marker.protected {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Protected Sync account state is missing; recovery is required",
            ));
        }
        Ok(raw)
    }
    fn require_settled(&self) -> io::Result<()> {
        if self.pending.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Inspect the pending protected Sync account write before continuing",
            ));
        }
        Ok(())
    }
}
fn corrupt() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Protected Sync account state is invalid or foreign",
    )
}
fn custody_error(_: io::Error) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Protected Sync account custody did not complete; inspect before retrying",
    )
}
