mod catalogue;
mod lifetime;
mod operations;
mod storage;
mod transfers;

use super::{Host, Result, error};
use crate::control::Control;
use catalogue::{Catalogue, Entry};
use lapis_app_core::vault::VaultRoot;
use lapis_app_protocol::{ErrorCode, ManagedVault};
use std::{io, path::Path, sync::Arc};
use uuid::Uuid;

pub(crate) enum Command {
    Create(String),
    Import(String, std::fs::File),
    Rename(Uuid, String),
    Archive(Uuid),
    Restore(Uuid),
}
pub(super) struct Management {
    control: Arc<Control>,
    storage: VaultRoot,
    pub catalogue: Catalogue,
    gate: tokio::sync::Mutex<()>,
    lifetime: lifetime::Lifetime,
}
impl Host {
    /// Installation startup only, before this host is exposed to HTTP callers.
    pub async fn enable_management(&self, control: Arc<Control>, profile: &Path) -> io::Result<()> {
        if self.management.get().is_some() {
            return Err(io::Error::other("Vault management is already selected"));
        }
        let storage = storage::open(profile)?;
        let existing = control.vaults().await?;
        for vault in &self.vaults {
            match existing
                .iter()
                .find(|row| row.vault.vault_id == vault.vault_id)
            {
                Some(row) if row.managed => {
                    return Err(io::Error::other(
                        "Configured vault identity conflicts with managed storage",
                    ));
                }
                Some(_) => (),
                None => {
                    control
                        .insert_vault(
                            vault,
                            false,
                            self.roots[&vault.vault_id].physical_identity()?,
                        )
                        .await?
                }
            }
        }
        let catalogue = Catalogue::default();
        let rows = control.vaults().await?;
        if rows.len() > 20 {
            return Err(io::Error::other("At most twenty vaults are supported"));
        }
        for row in rows {
            let root = if row.managed {
                storage::child(&storage, &format!("{}/content", row.vault.vault_id))?
            } else if let Some(root) = self.roots.get(&row.vault.vault_id) {
                root.try_clone()?
            } else {
                continue;
            };
            if root.physical_identity()? != row.identity {
                return Err(io::Error::other(
                    "Managed vault storage changed; restore the original directory",
                ));
            }
            if row.vault.archived_at.is_none() {
                self.runtime
                    .admit_root(row.vault.vault_id, root.try_clone()?)
                    .map_err(io::Error::other)?;
            } else if self.runtime.get(row.vault.vault_id).is_ok() {
                self.close(row.vault.vault_id)
                    .map_err(|error| io::Error::other(error.message))?;
            }
            catalogue
                .insert(Entry {
                    available: row.vault.archived_at.is_none(),
                    vault: row.vault,
                    root,
                })
                .map_err(|error| io::Error::other(error.message))?;
        }
        self.management
            .set(Arc::new(Management {
                control,
                storage,
                catalogue,
                gate: tokio::sync::Mutex::new(()),
                lifetime: lifetime::Lifetime::default(),
            }))
            .map_err(|_| io::Error::other("Vault management is already selected"))
    }
    pub fn manages_vaults(&self) -> bool {
        self.management.get().is_some()
    }

    pub(crate) async fn manage(self: &Arc<Self>, command: Command) -> Result<ManagedVault> {
        let manager = self
            .management
            .get()
            .ok_or_else(|| {
                error(
                    ErrorCode::Unsupported,
                    "Vault management requires the installation profile",
                )
            })?
            .clone();
        let accepted = manager.accept()?;
        let host = self.clone();
        tokio::spawn(async move {
            let _accepted = accepted;
            let _guard = manager.gate.lock().await;
            manager.execute(host, command).await
        })
        .await
        .map_err(|_| {
            error(
                ErrorCode::Io,
                "Vault management outcome is unavailable; refresh the catalogue",
            )
        })?
    }
}

fn store(_: io::Error) -> lapis_app_protocol::ServiceError {
    error(
        ErrorCode::Io,
        "Vault catalogue is unavailable; refresh before retrying",
    )
}
fn name(value: String) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.encode_utf16().count() > 200 || value.chars().any(char::is_control)
    {
        return Err(error(
            ErrorCode::InvalidRequest,
            "Vault name must contain 1 to 200 display characters",
        ));
    }
    Ok(value.into())
}
