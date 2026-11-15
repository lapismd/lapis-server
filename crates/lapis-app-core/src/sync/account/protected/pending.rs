use super::{HostSecretBytes, ProtectedAccountState, corrupt, custody_error, scope::Marker};
use sha2::{Digest, Sha256};
use std::io;
pub(super) struct Write {
    pub previous: Option<[u8; 32]>,
    pub desired: HostSecretBytes,
    pub marker: Marker,
}
pub(super) fn digest(bytes: &HostSecretBytes) -> [u8; 32] {
    Sha256::digest(bytes.expose_secret()).into()
}
impl ProtectedAccountState {
    pub(super) fn flush_pending(&mut self) -> io::Result<()> {
        self.owner.verify()?;
        let pending = self.pending.as_ref().ok_or_else(corrupt)?;
        self.owner.commit_marker(&pending.marker)?;
        // Do not use current_raw: first-write uncertainty can legitimately still
        // have no custody entry even though the durable marker expects one.
        let observed = self.custody.read(&self.owner.key).map_err(custody_error)?;
        let actual = observed.as_ref().map(digest);
        if actual != Some(digest(&pending.desired)) {
            if actual != pending.previous {
                return Err(corrupt());
            }
            self.custody
                .write(&self.owner.key, &pending.desired)
                .map_err(custody_error)?;
        }
        self.owner.verify_marker()?;
        self.pending = None;
        Ok(())
    }
}
