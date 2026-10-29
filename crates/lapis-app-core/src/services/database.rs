use super::{Client, Error, Result, VaultService, Work};
use crate::{database::Database, embedding::host::NativeEmbeddingHost, vault::VaultRoot};
use lapis_app_protocol::{
    ErrorCode, VaultPhase,
    database::{
        call::{DatabaseCall, DatabaseValue},
        changes::ChangeSet,
    },
};
use std::{path::Path, sync::Arc};

/// Native installation capability. No transport accepts profile or model paths.
pub struct DatabaseStorage {
    root: VaultRoot,
    embeddings: NativeEmbeddingHost,
}
impl DatabaseStorage {
    pub fn new(directory: &Path, embeddings: NativeEmbeddingHost) -> Result<Self> {
        if !directory.is_absolute()
            || !std::fs::symlink_metadata(directory).is_ok_and(|entry| entry.is_dir())
        {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "Database storage requires an existing physical profile directory",
            ));
        }
        Ok(Self {
            root: VaultRoot::open(directory)?,
            embeddings,
        })
    }
    pub(super) fn validate_vault(&self, vault: &VaultRoot) -> Result<()> {
        self.root.verify_physical_root()?;
        if self.root.path().starts_with(vault.path()) {
            return Err(Error::new(
                ErrorCode::PermissionDenied,
                "Application database storage must be outside admitted vaults",
            ));
        }
        Ok(())
    }
}

struct Opening<'a>(&'a VaultService);
impl Drop for Opening<'_> {
    fn drop(&mut self) {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.database_starting = false;
        state.advance_close();
        self.0.changed.notify_all();
    }
}

impl VaultService {
    pub(super) async fn database(self: &Arc<Self>) -> Result<Database> {
        let _gate = self.database_open.lock().await;
        let storage = self.storage.as_ref().ok_or_else(|| {
            Error::new(
                ErrorCode::Unsupported,
                "Native application database is not selected",
            )
        })?;
        {
            let mut state = self.lock()?;
            state.require_open()?;
            state.root.verify_physical_root()?;
            storage.validate_vault(&state.root)?;
            if let Some(error) = &state.database_error {
                return Err(Error(error.clone()));
            }
            if let Some(database) = &state.database {
                return Ok(database.clone());
            }
            state.database_starting = true;
        }
        let _opening = Opening(self);
        let database = Database::open_with_embeddings(
            storage.root.path(),
            self.id,
            storage.embeddings.clone(),
        )
        .await?;
        let completion = database.completion();
        {
            let mut state = self.lock()?;
            state.database = Some(database.clone());
            if state.phase != VaultPhase::Open {
                database.fence();
            }
        }
        let owner = Arc::downgrade(self);
        // This observer owns no admission sender or vault, so it cannot keep a
        // dropped runtime alive. Only native worker completion retires ownership.
        tokio::spawn(async move {
            let result = completion.wait().await;
            if let Some(owner) = owner.upgrade() {
                let mut state = owner
                    .state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                state.database = None;
                state.database_error = result.err().map(|error| error.0);
                state.advance_close();
                owner.changed.notify_all();
            }
        });
        Ok(database)
    }
}
impl Client {
    /// Passive, bounded commit observation. The receiver owns no writer or lease.
    pub async fn subscribe_to_database_changes(
        &self,
    ) -> Result<tokio::sync::broadcast::Receiver<ChangeSet>> {
        self.vault.lock()?.require_client(self.id)?;
        let opening = self.vault.begin_work()?;
        let database = self.vault.database().await?;
        let changes = database.subscribe_to_changes();
        drop(opening);
        Ok(changes)
    }

    pub async fn call_database(&self, request: DatabaseCall) -> Result<DatabaseValue> {
        self.vault.lock()?.require_client(self.id)?;
        let opening = self.vault.begin_work()?;
        let database = self.vault.database().await?;
        // Once opened, the worker owns admission and drain. A parked response
        // waiter must not hold a native work lease or postpone vault closure.
        drop(opening);
        database.call_from_client(request).await
    }
}
impl Work {
    /// Accepted native work shares the same database and close fence as clients.
    pub async fn call_database(&self, request: DatabaseCall) -> Result<DatabaseValue> {
        self.vault.database().await?.call(request).await
    }
}
