mod broker;
mod control;
mod credentials;
mod discovery;
mod executor;
mod handoff;
mod network;
mod network_policy;
mod snapshot;
mod storage;
mod worker;

use super::{Client, Error, Result, VaultService, Work};
pub use credentials::{CredentialScope, credential_namespace};
use lapis_app_protocol::ErrorCode;
use lapis_plugin_jobs::*;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
pub use storage::JobStorage;
pub type NotificationDelivery = Arc<
    dyn Fn(
            &lapis_app_protocol::database::state::NotificationRecord,
        ) -> std::result::Result<(), String>
        + Send
        + Sync,
>;
use uuid::Uuid;

fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InvalidRequest, message)
}
fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub(super) struct Owner {
    executor: Arc<executor::Executor>,
    notification_delivery: Option<NotificationDelivery>,
    credential_scope: Option<CredentialScope>,
    enabled: Arc<AtomicBool>,
    stored: Mutex<storage::Stored>,
    stopped: AtomicBool,
    cancelled: AtomicBool,
    changed: Condvar,
    wake: Mutex<()>,
}

impl Owner {
    pub(super) fn start(vault: &Arc<VaultService>, storage: &JobStorage) -> Result<Arc<Self>> {
        let stored = storage.open(vault)?;
        let owner = Arc::new(Self {
            enabled: stored.enabled.clone(),
            notification_delivery: storage.notification_delivery.clone(),
            credential_scope: storage.credential_scope.clone(),
            executor: storage.executor.clone(),
            stored: Mutex::new(stored),
            stopped: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            changed: Condvar::new(),
            wake: Mutex::new(()),
        });
        let weak_vault = Arc::downgrade(vault);
        let weak_owner = Arc::downgrade(&owner);
        std::thread::Builder::new()
            .name("lapis-plugin-jobs".into())
            .spawn(move || worker::run(weak_vault, weak_owner))?;
        Ok(owner)
    }
    fn store(&self) -> Result<std::sync::MutexGuard<'_, storage::Stored>> {
        self.stored.lock().map_err(|_| Error::closed())
    }
    pub(super) fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        self.cancelled.store(true, Ordering::Release);
        self.changed.notify_all();
    }
}

impl VaultService {
    pub fn has_background_jobs(&self) -> bool {
        self.jobs
            .lock()
            .ok()
            .and_then(|owner| owner.clone())
            .is_some_and(|owner| {
                !owner.stopped.load(Ordering::Acquire) && owner.enabled.load(Ordering::Acquire)
            })
    }
    pub(super) fn jobs(&self) -> Result<Arc<Owner>> {
        self.jobs
            .lock()
            .map_err(|_| Error::closed())?
            .clone()
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::Unsupported,
                    "Background jobs require native installation storage",
                )
            })
    }
    pub(super) fn stop_jobs(&self) {
        if let Ok(Some(owner)) = self.jobs.lock().map(|owner| owner.clone()) {
            owner.stop();
        }
    }
    pub(super) fn retire_jobs(&self) -> Result<()> {
        if let Some(owner) = self.jobs.lock().map_err(|_| Error::closed())?.take() {
            owner.stop();
            owner.store()?.store.retire()?;
        }
        Ok(())
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.stop();
    }
}
