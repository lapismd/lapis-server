use super::{Host, Principal, Result};
use crate::config::Config;
use lapis_app_core::{
    embedding::host::NativeEmbeddingHost,
    services::{BackupStorage, DatabaseStorage, Runtime},
    vault::VaultRoot,
};
use lapis_app_protocol::database::call::{DatabaseCall, DatabaseValue};
use std::{io, path::Path};
use uuid::Uuid;

impl Host {
    pub(crate) fn observation_client(
        &self,
        id: Uuid,
        principal: Principal,
        vault: Uuid,
        generation: Uuid,
    ) -> Result<std::sync::Arc<lapis_app_core::services::Client>> {
        let client = self.client(id, principal)?;
        let descriptor = client.descriptor();
        if descriptor.vault_id != vault || descriptor.generation != generation {
            return Err(super::error(
                lapis_app_protocol::ErrorCode::PermissionDenied,
                "Database observation scope is not admitted",
            ));
        }
        Ok(client)
    }
    /// Installation-only selection; HTTP callers never supply this profile.
    pub fn with_profile(config: Config, profile: &Path) -> io::Result<Self> {
        Self::with_profile_and_agents(config, profile, Vec::new())
    }
    pub fn with_profile_and_agents(
        config: Config,
        profile: &Path,
        agents: Vec<super::turns::AgentConfig>,
    ) -> io::Result<Self> {
        Self::profile(config, profile, agents, false)
    }
    /// Explicit installation selection. Browser commands cannot select storage.
    pub fn with_indexed_history(
        config: Config,
        profile: &Path,
        agents: Vec<super::turns::AgentConfig>,
    ) -> io::Result<Self> {
        Self::profile(config, profile, agents, true)
    }
    fn profile(
        config: Config,
        profile: &Path,
        agents: Vec<super::turns::AgentConfig>,
        indexed_history: bool,
    ) -> io::Result<Self> {
        if !config.vaults.is_empty() {
            config.validate()?;
        }
        let (database, backup) = storage(profile, &config)?;
        let mut runtime = Runtime::with_database(database)
            .with_backup(backup.with_history_indexing(indexed_history))
            .with_tasks(super::tasks::storage::select(profile)?);
        if let Some(storage) = super::turns::storage::select(profile, &config, agents)? {
            runtime = runtime.with_turns(storage);
        }
        let mut host = Self::with_runtime(config, runtime)?;
        host.automatic_backup = true;
        Ok(host)
    }
    pub async fn database(
        &self,
        client: Uuid,
        principal: Principal,
        call: DatabaseCall,
    ) -> Result<DatabaseValue> {
        self.client(client, principal)?
            .call_database(call)
            .await
            .map_err(|error| error.0)
    }
}

fn storage(profile: &Path, config: &Config) -> io::Result<(DatabaseStorage, BackupStorage)> {
    if !profile.is_absolute()
        || !std::fs::symlink_metadata(profile).is_ok_and(|entry| entry.is_dir())
    {
        return Err(io::Error::other(
            "Server profile requires an existing physical directory",
        ));
    }
    let root = VaultRoot::open(profile)?;
    let database = root.path().join("databases");
    let models = root.path().join("models");
    let history = root.path().join("vault-history");
    for vault in &config.vaults {
        let vault = vault.root.canonicalize()?;
        if root.path().starts_with(&vault)
            || [&database, &models, &history]
                .iter()
                .any(|storage| storage.starts_with(&vault) || vault.starts_with(storage))
        {
            return Err(io::Error::other(
                "Server database/cache/history storage must stay outside vault content",
            ));
        }
    }
    for name in ["databases", "models", "vault-history"] {
        if let Err(error) = root.mkdir(name, false)
            && error.kind() != io::ErrorKind::AlreadyExists
        {
            return Err(error);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            root.open_directory(name)?
                .open(".")?
                .into_std()
                .set_permissions(std::fs::Permissions::from_mode(0o700))?;
        }
    }
    root.verify_physical_root()?;
    let embeddings = NativeEmbeddingHost::managed(&models).map_err(io::Error::other)?;
    let storage = DatabaseStorage::new(&database, embeddings).map_err(io::Error::other)?;
    root.verify_physical_root()?;
    let backup = BackupStorage::new(&history).map_err(io::Error::other)?;
    Ok((storage, backup))
}
