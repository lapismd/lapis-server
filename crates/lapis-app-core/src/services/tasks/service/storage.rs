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
    pub(super) fn configuration(
        &self,
        vault: uuid::Uuid,
    ) -> Result<Option<lapis_app_protocol::native_tasks::NativeTaskConfiguration>> {
        let path = format!("{vault}-configuration.json");
        self.root.verify_physical_root()?;
        if self.root.stat(&path)?.is_none() {
            return Ok(None);
        }
        let bytes = self.root.read_range(&path, 0, 65537)?;
        if bytes.len() > 65536 {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "Stored Task configuration exceeds bounds",
            ));
        }
        serde_json::from_slice(&bytes).map_err(|_| {
            Error::new(
                ErrorCode::InvalidRequest,
                "Invalid stored Task configuration",
            )
        })
    }
    pub(super) fn save_configuration(
        &self,
        vault: uuid::Uuid,
        configuration: Option<&lapis_app_protocol::native_tasks::NativeTaskConfiguration>,
    ) -> Result<()> {
        let bytes = serde_json::to_vec(&configuration)
            .map_err(|_| Error::new(ErrorCode::InvalidRequest, "Invalid Task configuration"))?;
        self.root.write(
            &format!("{vault}-configuration.json"),
            &bytes,
            &crate::vault::WriteOptions::default(),
        )?;
        Ok(())
    }
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
