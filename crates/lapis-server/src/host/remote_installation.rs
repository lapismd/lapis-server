//! Installation-only credential composition for the single authenticated owner.
mod storage;
#[cfg(test)]
mod tests;
use crate::{config::Config, credentials::CredentialStore};
use lapis_app_core::{services::RemoteStorage, vault::VaultRoot};
use std::{io, path::Path, path::PathBuf, sync::Arc};

/// Native startup inputs. No HTTP DTO accepts paths, keys or owner identity.
#[derive(Default)]
pub struct RemoteInstallation {
    pub key_file: Option<PathBuf>,
    pub known_hosts: Option<PathBuf>,
    pub github_client: Option<String>,
    pub ssh_available: bool,
}
impl RemoteInstallation {
    pub(super) fn select(self, profile: &Path, config: &Config) -> io::Result<RemoteStorage> {
        let profile = VaultRoot::open(profile)?;
        let vaults = config
            .vaults
            .iter()
            .map(|v| v.root.canonicalize())
            .collect::<io::Result<Vec<_>>>()?;
        if vaults.iter().any(|v| profile.path().starts_with(v)) {
            return Err(io::Error::other(
                "Repository installation storage must stay outside vaults",
            ));
        }
        if let Some(key) = &self.key_file {
            // Missing/wrong keys lock custody. An unsafe key location is a configuration error.
            if !key.is_absolute()
                || key
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err(io::Error::other(
                    "Repository key file requires an absolute operator path",
                ));
            }
            let physical = key.canonicalize().unwrap_or_else(|_| key.clone());
            if physical.starts_with(profile.path())
                || vaults.iter().any(|v| physical.starts_with(v))
            {
                return Err(io::Error::other(
                    "Repository key must be mounted separately from profile and vaults",
                ));
            }
        }
        let root = storage::child(&profile, "repository-remotes")?;
        let identity = storage::identity(&root)?;
        let secrets = storage::child(&root, "credentials")?;
        let accounts = storage::child(&root, "accounts")?;
        let selection = storage::child(&root, "selection")?;
        let store = Arc::new(CredentialStore::open(
            secrets.path(),
            identity.installation,
            self.key_file.as_deref(),
        ));
        let credentials = store.remote_credentials(
            identity.owner,
            accounts.path().into(),
            self.github_client,
            self.ssh_available,
        )?;
        let mut remote = RemoteStorage::new(selection.path(), credentials)
            .map_err(io::Error::other)?
            .with_transfers(true);
        if let Some(path) = self.known_hosts {
            remote = remote.with_known_hosts(&path).map_err(io::Error::other)?;
            // Managed vaults live under profile/vaults and may be restored later.
            let trust = path.canonicalize()?;
            if trust.starts_with(profile.path().join("vaults"))
                || vaults.iter().any(|v| trust.starts_with(v))
            {
                return Err(io::Error::other(
                    "SSH trust must stay outside vault content",
                ));
            }
        }
        root.verify_physical_root()?;
        profile.verify_physical_root()?;
        Ok(remote)
    }
}
