use super::{AgentConfig, policy::Policy};
use crate::config::Config;
use lapis_app_core::{services::TurnStorage, vault::VaultRoot};
use std::{io, path::Path, sync::Arc};

pub(in crate::host) fn select(
    profile: &Path,
    config: &Config,
    agents: Vec<AgentConfig>,
) -> io::Result<Option<TurnStorage>> {
    if agents.is_empty() {
        return Ok(None);
    }
    let root = VaultRoot::open(profile)?;
    let mut forbidden = config
        .vaults
        .iter()
        .map(|vault| vault.root.canonicalize())
        .collect::<io::Result<Vec<_>>>()?;
    forbidden.push(root.path().join("vaults"));
    let policy = Policy::new(agents, forbidden)?;
    match root.mkdir("turns", false) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    root.open_directory("")?.open(".")?.sync_all()?;
    use std::os::unix::fs::PermissionsExt;
    root.open_directory("turns")?
        .open(".")?
        .into_std()
        .set_permissions(std::fs::Permissions::from_mode(0o700))?;
    let storage = TurnStorage::new(
        &root.path().join("turns"),
        Arc::new(policy),
        tokio::runtime::Handle::try_current().map_err(io::Error::other)?,
    )
    .map_err(io::Error::other)?;
    root.verify_physical_root()?;
    Ok(Some(storage))
}
