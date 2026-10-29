use serde::Deserialize;
use std::{io, path::PathBuf};
use uuid::Uuid;

/// Operator input only. This type is never accepted by an HTTP route.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub vaults: Vec<VaultConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VaultConfig {
    pub id: Uuid,
    pub name: String,
    pub root: PathBuf,
}

impl Config {
    pub fn validate(&self) -> io::Result<()> {
        if self.vaults.is_empty() || self.vaults.len() > 20 {
            return Err(io::Error::other("Configure between one and twenty vaults"));
        }
        let mut ids = std::collections::HashSet::new();
        for vault in &self.vaults {
            if vault.id.is_nil()
                || !ids.insert(vault.id)
                || vault.name.trim().is_empty()
                || vault.name.len() > 256
                || !vault.root.is_absolute()
            {
                return Err(io::Error::other("Invalid managed vault configuration"));
            }
        }
        Ok(())
    }
}
