use super::corrupt;
use crate::{
    sync::private::open_private_leaf_admitted,
    vault::{VaultRoot, WriteOptions},
};
use same_file::Handle;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read},
};
use uuid::Uuid;
const MARKER: &str = "binding.json";
const MAX_MARKER_BYTES: usize = 8 * 1024;
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Scope {
    pub version: u32,
    pub profile: Uuid,
    pub vault: Uuid,
    pub origin: String,
    pub profile_fs: [u64; 2],
    pub vault_fs: [u64; 2],
    pub lock_fs: [u64; 2],
    pub client: Uuid,
    pub binding: Uuid,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Marker {
    pub scope: Scope,
    pub protected: bool,
}
pub(super) struct Owner {
    profile: VaultRoot,
    vault: VaultRoot,
    root: VaultRoot,
    lock: File,
    lock_identity: Handle,
    closed: bool,
    expected: Scope,
    pub key: String,
    pub fresh_lock: bool,
    pub marker: Marker,
    pub marker_bytes: Option<Vec<u8>>,
}
impl Owner {
    pub fn open(
        profile: &VaultRoot,
        vault: &VaultRoot,
        profile_id: Uuid,
        vault_id: Uuid,
        origin: &str,
    ) -> io::Result<Self> {
        if profile_id.is_nil()
            || vault_id.is_nil()
            || profile.path().starts_with(vault.path())
            || vault.path().starts_with(profile.path())
        {
            return Err(corrupt());
        }
        profile.verify_physical_root()?;
        vault.verify_physical_root()?;
        let (pd, pi) = profile.physical_identity()?;
        let (vd, vi) = vault.physical_identity()?;
        let mut expected = Scope {
            version: 1,
            profile: profile_id,
            vault: vault_id,
            origin: origin.into(),
            profile_fs: [pd, pi],
            vault_fs: [vd, vi],
            lock_fs: [0, 0],
            client: Uuid::new_v4(),
            binding: Uuid::new_v4(),
        };
        let mut hash = Sha256::new();
        hash.update(b"lapis-sync-account-v1\0");
        hash.update(profile_id.as_bytes());
        hash.update(vault_id.as_bytes());
        hash.update(origin.as_bytes());
        let destination = format!("{:x}", hash.finalize());
        let (_, _, directory, path) =
            open_private_leaf_admitted(profile, &vault_id.to_string(), &destination, "account")?;
        let root = VaultRoot::from_capability(path, directory)?;
        let fresh_lock = match root.create_file(".owner.lock", &b""[..], &WriteOptions::default()) {
            Ok(()) => true,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
            Err(error) => return Err(error),
        };
        let lock = root.open_file(".owner.lock")?.into_std();
        lock.try_lock()?;
        let lock_identity = Handle::from_file(lock.try_clone()?)?;
        use std::os::unix::fs::MetadataExt as _;
        let metadata = lock.metadata()?;
        expected.lock_fs = [metadata.dev(), metadata.ino()];
        let mut owner = Self {
            profile: profile.try_clone()?,
            vault: vault.try_clone()?,
            root,
            lock,
            lock_identity,
            closed: false,
            fresh_lock,
            marker: Marker {
                scope: expected.clone(),
                protected: false,
            },
            marker_bytes: None,
            expected,
            key: format!("lapis.sync.account.v1.{destination}"),
        };
        if let Some(bytes) = owner.read_marker()? {
            let marker: Marker = serde_json::from_slice(&bytes).map_err(|_| corrupt())?;
            owner.validate_scope(&marker.scope)?;
            owner.marker = marker;
            owner.marker_bytes = Some(bytes);
        }
        owner.verify()?;
        Ok(owner)
    }
    pub fn validate_scope(&self, scope: &Scope) -> io::Result<()> {
        let expected = &self.expected;
        if scope.version != 1
            || scope.profile != expected.profile
            || scope.vault != expected.vault
            || scope.origin != expected.origin
            || scope.profile_fs != expected.profile_fs
            || scope.vault_fs != expected.vault_fs
            || scope.lock_fs != expected.lock_fs
            || scope.client.is_nil()
            || scope.binding.is_nil()
        {
            return Err(corrupt());
        }
        Ok(())
    }
    pub fn verify(&self) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "Protected Sync account owner is closed",
            ));
        }
        self.profile.verify_physical_root()?;
        self.vault.verify_physical_root()?;
        self.root.verify_physical_root()?;
        if Handle::from_file(self.root.open_file(".owner.lock")?.into_std())? != self.lock_identity
        {
            return Err(corrupt());
        }
        Ok(())
    }
    pub fn verify_marker(&self) -> io::Result<()> {
        self.verify()?;
        if self.read_marker()? != self.marker_bytes {
            return Err(corrupt());
        }
        Ok(())
    }
    fn read_marker(&self) -> io::Result<Option<Vec<u8>>> {
        let file = match self.root.open_file(MARKER) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        file.take(MAX_MARKER_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_MARKER_BYTES {
            return Err(corrupt());
        }
        Ok(Some(bytes))
    }
    pub fn commit_marker(&mut self, marker: &Marker) -> io::Result<()> {
        self.verify()?;
        self.validate_scope(&marker.scope)?;
        let desired = serde_json::to_vec(marker).map_err(|_| corrupt())?;
        if desired.len() > MAX_MARKER_BYTES {
            return Err(corrupt());
        }
        let current = self.read_marker()?;
        if current != self.marker_bytes && current.as_deref() != Some(&desired) {
            return Err(corrupt());
        }
        if current != self.marker_bytes || current.as_deref() != Some(&desired) {
            // A previous replace may have succeeded before a failed durability
            // acknowledgement. Inspect its bytes, then recommit the same marker.
            self.root.write_if_unchanged(
                MARKER,
                &desired[..],
                current.as_deref(),
                &WriteOptions::default(),
            )?;
        }
        self.verify()?;
        self.marker = marker.clone();
        self.marker_bytes = Some(desired);
        Ok(())
    }
    pub fn close(&mut self) -> io::Result<()> {
        if !self.closed {
            self.lock.unlock()?;
            self.closed = true;
        }
        Ok(())
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.lock.unlock();
        }
    }
}
