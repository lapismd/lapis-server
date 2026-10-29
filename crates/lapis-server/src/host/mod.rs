mod attachments;
mod backup;
mod database;
mod history_index;
mod history_reader;
mod indexing;
mod management;
mod repository_backup;
mod repository_restore;
mod repository_sources;
mod tasks;
pub mod turns;
pub(crate) use management::Command as ManagementCommand;

use crate::config::Config;
use lapis_app_core::services::{Client, Runtime};
use lapis_app_protocol::{ClientDescriptor, ErrorCode, ManagedVault, Reply, Request, ServiceError};
use std::{
    collections::HashMap,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, ServiceError>;
pub const CLIENT_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Principal {
    Token,
    Session(Uuid),
}

pub struct Host {
    ready: AtomicBool,
    runtime: Runtime,
    vaults: Vec<ManagedVault>,
    clients: attachments::Attachments,
    automatic_backup: bool,
    roots: HashMap<Uuid, lapis_app_core::vault::VaultRoot>,
    management: OnceLock<Arc<management::Management>>,
}

pub fn error(code: ErrorCode, message: &str) -> ServiceError {
    ServiceError {
        code,
        message: message.into(),
    }
}

impl Host {
    pub fn new(config: Config) -> std::io::Result<Self> {
        config.validate()?;
        Self::with_runtime(config, Runtime::default())
    }
    fn with_runtime(config: Config, runtime: Runtime) -> std::io::Result<Self> {
        if !config.vaults.is_empty() {
            config.validate()?;
        }
        let mut vaults = Vec::new();
        let mut roots = HashMap::new();
        for vault in config.vaults {
            let root = lapis_app_core::vault::VaultRoot::open(&vault.root)?;
            runtime
                .admit_root(vault.id, root.try_clone()?)
                .map_err(std::io::Error::other)?;
            roots.insert(vault.id, root);
            vaults.push(ManagedVault {
                vault_id: vault.id,
                name: vault.name,
                archived_at: None,
            });
        }
        Ok(Self {
            ready: AtomicBool::new(true),
            runtime,
            vaults,
            clients: attachments::Attachments::default(),
            automatic_backup: false,
            roots,
            management: OnceLock::new(),
        })
    }

    pub fn vaults(&self) -> Result<Vec<ManagedVault>> {
        match self.management.get() {
            Some(manager) => manager.catalogue.list(),
            None => Ok(self.vaults.clone()),
        }
    }

    pub fn attach(&self, vault: Uuid, principal: Principal) -> Result<ClientDescriptor> {
        self.expire_idle(Instant::now())?;
        let _admission = self
            .management
            .get()
            .map(|manager| manager.catalogue.admission(vault))
            .transpose()?;
        let client = self
            .runtime
            .get(vault)
            .and_then(|owner| owner.attach())
            .map_err(|e| e.0)?;
        let descriptor = client.descriptor();
        self.clients.insert(Arc::new(client), principal)?;
        Ok(descriptor)
    }

    pub fn execute(&self, client: Uuid, principal: Principal, request: Request) -> Result<Reply> {
        // Do not hold the attachment registry during file IO or service drain.
        let client = self.client(client, principal)?;
        Ok(client.execute(request))
    }

    pub fn detach(&self, client: Uuid, principal: Principal) -> Result<()> {
        self.expire_idle(Instant::now())?;
        self.clients.remove(client, principal)
    }

    pub fn heartbeat(&self, client: Uuid, principal: Principal) -> Result<()> {
        self.client(client, principal)?;
        Ok(())
    }

    pub fn expire_idle(&self, now: Instant) -> Result<usize> {
        self.clients.expire(now)
    }

    pub(super) fn client(&self, id: Uuid, principal: Principal) -> Result<Arc<Client>> {
        self.clients.get(id, principal)
    }

    pub fn detach_principal(&self, principal: Principal) -> Result<()> {
        self.clients.retire_principal(principal)
    }

    pub fn close(&self, vault: Uuid) -> Result<()> {
        self.runtime.close_vault(vault).map_err(|e| e.0)?;
        self.clients.retire(Some(vault))?;
        Ok(())
    }

    pub(crate) fn ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    pub fn shutdown(&self) -> Result<()> {
        self.ready.store(false, Ordering::Release);
        let result = self.shutdown_owned();
        if result.is_err() {
            self.ready.store(true, Ordering::Release);
        }
        result
    }

    fn shutdown_owned(&self) -> Result<()> {
        if let Some(manager) = self.management.get() {
            manager.pause_and_drain()?;
        }
        if let Err(error) = self.runtime.shutdown() {
            if let Some(manager) = self.management.get() {
                manager.resume();
            }
            return Err(error.0);
        }
        self.clients.retire(None)?;
        Ok(())
    }
}
