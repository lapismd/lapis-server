//! One admitted vault owns journal and policy; clients only call bounded controls.
mod commands;
mod errors;
mod observations;
mod projection;
mod storage;
#[cfg(test)]
mod tests;
mod validation;
pub use storage::TaskStorage;

use crate::services::{Client, Error, Result, TaskLifecycleJournal, VaultService};
use lapis_app_protocol::{ErrorCode, native_tasks::*};
use std::sync::{Arc, Mutex};

pub(in crate::services) struct Owner {
    data: Mutex<Data>,
}
struct Data {
    journal: TaskLifecycleJournal,
    configuration: Option<NativeTaskConfiguration>,
}
fn unavailable() -> Error {
    Error::new(
        ErrorCode::Io,
        "Native Task service is unavailable; inspect before retrying",
    )
}
fn unselected() -> Error {
    Error::new(ErrorCode::Unsupported, "Native Tasks are not selected")
}
fn storage(vault: &VaultService) -> Result<Arc<TaskStorage>> {
    let storage = vault.task_storage.as_ref().ok_or_else(unselected)?.clone();
    storage.validate_vault(&vault.lock()?.root)?;
    Ok(storage)
}
fn owner(vault: &Arc<VaultService>) -> Result<Arc<Owner>> {
    let storage = storage(vault)?;
    let mut slot = vault.tasks.lock().map_err(|_| unavailable())?;
    if let Some(owner) = slot.as_ref() {
        return Ok(owner.clone());
    }
    let journal =
        TaskLifecycleJournal::open(storage.root.path(), vault).map_err(|_| unavailable())?;
    let owner = Arc::new(Owner {
        data: Mutex::new(Data {
            journal,
            configuration: None,
        }),
    });
    *slot = Some(owner.clone());
    Ok(owner)
}
impl Client {
    /// Dropping the response leaves this blocking operation and its Work owned by the vault.
    pub async fn control_tasks(&self, command: NativeTaskCommand) -> Result<NativeTaskValue> {
        self.vault.lock()?.require_client(self.id)?;
        validation::command(&command)?;
        let work = self.vault.begin_work()?;
        let vault = self.vault.clone();
        let runtime = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || commands::execute(vault, work, command, runtime))
            .await
            .map_err(|_| unavailable())?
    }
}
impl VaultService {
    pub(in crate::services) fn retire_tasks(&self) -> Result<()> {
        let mut slot = self.tasks.lock().map_err(|_| unavailable())?;
        if let Some(owner) = slot.as_ref() {
            owner
                .data
                .lock()
                .map_err(|_| unavailable())?
                .journal
                .retire()
                .map_err(|_| unavailable())?;
        }
        *slot = None;
        Ok(())
    }
}
