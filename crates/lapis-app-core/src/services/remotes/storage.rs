use super::*;
use crate::vault::VaultRoot;
use cap_std::fs::MetadataExt as _;
use std::{io, path::Path, sync::Mutex};

/// Passive host-owned inventory. Implementations must not transfer or return secrets.
pub trait RemoteCredentials: Send + Sync {
    fn inventory(&self) -> Result<RemoteSettings>;
}
/// Installation selection only. No request can choose these native paths/providers.
pub struct RemoteStorage {
    root: VaultRoot,
    pub(super) credentials: Arc<dyn RemoteCredentials>,
    pub(super) gate: Mutex<()>,
}
impl RemoteStorage {
    pub fn new(directory: &Path, credentials: Arc<dyn RemoteCredentials>) -> Result<Self> {
        let root = VaultRoot::open(directory)?;
        if root.directory_handle()?.dir_metadata()?.mode() & 0o077 != 0 {
            return Err(Error::denied());
        }
        Ok(Self {
            root,
            credentials,
            gate: Mutex::new(()),
        })
    }
    pub(in crate::services) fn validate_vault(&self, vault: &VaultRoot) -> Result<()> {
        self.root.verify_physical_root()?;
        if self.root.path().starts_with(vault.path()) || vault.path().starts_with(self.root.path())
        {
            return Err(Error::denied());
        }
        Ok(())
    }
    pub(super) fn selection(&self, vault: &super::VaultService) -> Result<Selection> {
        let state = vault.lock()?;
        state.root.verify_physical_root()?;
        self.validate_vault(&state.root)?;
        let directory = vault.id.to_string();
        match self.root.mkdir(&directory, false) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        use std::os::unix::fs::PermissionsExt as _;
        let handle = self.root.open_directory(&directory)?.open(".")?.into_std();
        handle.set_permissions(std::fs::Permissions::from_mode(0o700))?;
        let private = VaultRoot::open(&self.root.path().join(directory))?;
        let selection = Selection::open(private, &state.root)?;
        self.root.verify_physical_root()?;
        Ok(selection)
    }
}
