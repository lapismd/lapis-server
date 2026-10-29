use crate::{
    services::{Error, Result},
    vault::VaultRoot,
};
use lapis_app_protocol::ErrorCode;
use std::path::Path;

/// Installation-owned Tasks journal root. It is never a protocol value.
pub struct TaskStorage {
    pub(super) root: VaultRoot,
}
impl TaskStorage {
    pub fn new(directory: &Path) -> Result<Self> {
        if !directory.is_absolute() || !std::fs::symlink_metadata(directory)?.is_dir() {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "Native Tasks require a physical profile",
            ));
        }
        Ok(Self {
            root: VaultRoot::open(directory)?,
        })
    }
    pub(crate) fn validate_vault(&self, vault: &VaultRoot) -> Result<()> {
        self.root.verify_physical_root()?;
        vault.verify_physical_root()?;
        if self.root.path().starts_with(vault.path()) || vault.path().starts_with(self.root.path())
        {
            return Err(Error::new(
                ErrorCode::PermissionDenied,
                "Native Task authority must stay outside vault content",
            ));
        }
        Ok(())
    }
}
