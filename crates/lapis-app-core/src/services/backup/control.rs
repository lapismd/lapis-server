use super::{Owner, commands, configuration, dto, failure};
use crate::services::{Client, Error, Result, VaultService};
use lapis_app_protocol::{ErrorCode, repository_backup::*};
use std::sync::Arc;

impl Client {
    /// Transitions are accepted native work. A discarded response does not
    /// abandon a saved selection or coordinator drain.
    pub async fn control_repository_backup(
        &self,
        command: RepositoryBackupCommand,
    ) -> Result<RepositoryBackupSnapshot> {
        self.vault.lock()?.require_client(self.id)?;
        let work = self.vault.begin_work()?;
        let vault = self.vault.clone();
        let ticket = vault.backup.reserve()?;
        ticket
            .run(work, move || {
                let storage = vault.backup_storage.as_ref().ok_or_else(|| {
                    Error::new(ErrorCode::Unsupported, "Automatic backup is not selected")
                })?;
                // Same per-vault gate as native start/disable. Status is a passive
                // barrier behind an already accepted transition, never a restart.
                let mut slot = vault.backup.lock().map_err(|_| failure())?;
                match command {
                    RepositoryBackupCommand::Status {} => {}
                    RepositoryBackupCommand::SetEnabled { enabled, expected } => {
                        configuration::preflight(&vault, &expected)?;
                        if !enabled {
                            commands::stop(&mut slot)?;
                        }
                        configuration::set(&vault, enabled, &expected)?;
                        if enabled && slot.is_none() {
                            *slot = Some(Arc::new(storage.start(&vault)?));
                        }
                    }
                    RepositoryBackupCommand::Capture {} => {
                        if !configuration::read(&vault)?.enabled {
                            return Err(Error::new(
                                ErrorCode::Unsupported,
                                "Automatic backup is disabled",
                            ));
                        }
                        if let Some(owner) = slot.as_ref() {
                            let engine = owner.engine.blocking_lock();
                            tokio::runtime::Builder::new_current_thread()
                                .enable_all()
                                .build()
                                .map_err(|_| failure())?
                                .block_on(engine.capture())
                                .map_err(|_| failure())?;
                        } else {
                            // Explicit manual capture reuses the ordinary native
                            // initialization, which already captures once.
                            *slot = Some(Arc::new(storage.start(&vault)?));
                        }
                    }
                }
                snapshot(&vault, slot.as_deref())
            })
            .await
    }
}
fn snapshot(vault: &VaultService, owner: Option<&Owner>) -> Result<RepositoryBackupSnapshot> {
    let configuration = configuration::read(vault)?;
    let status = match owner {
        Some(owner) => dto::status(owner.status.borrow().clone()),
        None if !configuration.enabled => RepositoryBackupStatus {
            phase: RepositoryBackupPhase::Closed,
            checkpoint: None,
            error: None,
        },
        None => RepositoryBackupStatus {
            phase: RepositoryBackupPhase::Failed,
            checkpoint: None,
            error: Some("Automatic backup is not running; retry capture or selection".into()),
        },
    };
    Ok(RepositoryBackupSnapshot {
        enabled: configuration.enabled,
        configuration: configuration.version,
        status,
    })
}
