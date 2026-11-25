use super::*;
use crate::{
    record_store::{RecordStore, StoredRecord},
    vault::VaultRoot,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{io, path::Path};

pub struct JobStorage {
    pub(super) executor: Arc<executor::Executor>,
    pub(super) notification_delivery: Option<super::NotificationDelivery>,
    root: VaultRoot,
    runner: String,
}

impl JobStorage {
    pub fn with_notification_delivery(mut self, delivery: super::NotificationDelivery) -> Self {
        self.notification_delivery = Some(delivery);
        self
    }
    pub(in crate::services) fn eligible_vault_ids(&self) -> Result<Vec<Uuid>> {
        use std::io::Read;
        self.root.verify_physical_root()?;
        let mut output = Vec::new();
        for directory in self.root.list("")?.folders {
            let Ok(id) = Uuid::parse_str(&directory) else {
                continue;
            };
            let root = self.root.open_child(&directory)?;
            let files: Vec<_> = root
                .list("")?
                .files
                .into_iter()
                .filter(|path| path.ends_with(".json"))
                .collect();
            if files.is_empty() {
                continue;
            }
            if files.len() != 1 {
                return Err(invalid("Invalid background job catalog"));
            }
            let mut bytes = Vec::new();
            root.open_file(&files[0])?
                .take(Catalog::MAX_RECORD_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > Catalog::MAX_RECORD_BYTES {
                return Err(invalid("Job catalog exceeds bounds"));
            }
            let catalog: Catalog = serde_json::from_slice(&bytes)
                .map_err(|_| invalid("Invalid background job catalog"))?;
            catalog.validate(id, catalog.id)?;
            if catalog.runner != self.runner {
                return Err(invalid("Job runner identity changed"));
            }
            if catalog.trusted
                && !catalog.safe_mode
                && catalog.jobs.iter().any(|job| job.configuration.enabled)
            {
                output.push(id);
            }
        }
        Ok(output)
    }
    pub fn new(directory: &Path) -> Result<Self> {
        let root = VaultRoot::open(directory)?;
        let identity = "installation.json";
        if root.stat(identity)?.is_none() {
            let bytes = serde_json::to_vec(&Uuid::new_v4().to_string())
                .map_err(|_| invalid("Invalid runner identity"))?;
            match root.create_file(identity, bytes.as_slice(), &Default::default()) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        let runner: String = serde_json::from_str(&root.read_text(identity)?)
            .map_err(|_| invalid("Invalid job runner identity"))?;
        Uuid::parse_str(&runner).map_err(|_| invalid("Invalid job runner identity"))?;
        Ok(Self {
            root,
            runner,
            executor: executor::Executor::new()?,
            notification_delivery: None,
        })
    }
    pub(in crate::services) fn validate(&self, content: &VaultRoot) -> Result<()> {
        self.root.verify_physical_root()?;
        if self.root.path().starts_with(content.path())
            || content.path().starts_with(self.root.path())
        {
            return Err(invalid("Job storage must stay outside vault content"));
        }
        Ok(())
    }
    pub(super) fn open(&self, vault: &Arc<VaultService>) -> Result<Stored> {
        let state = vault.lock()?;
        self.validate(&state.root)?;
        let store = RecordStore::<Catalog>::open(self.root.path(), vault.id, &state.root)?;
        let ids = store.ids()?;
        let (mut catalog, bytes) = if let Some(id) = ids.first() {
            store
                .read(*id)?
                .ok_or_else(|| invalid("Job catalog disappeared"))?
        } else {
            let catalog = Catalog {
                id: Uuid::new_v4(),
                vault: vault.id,
                schema: 1,
                revision: 0,
                runner: self.runner.clone(),
                trusted: true,
                safe_mode: false,
                jobs: Vec::new(),
                imports: Vec::new(),
            };
            let bytes = store.write(&catalog, None)?;
            (catalog, bytes)
        };
        if catalog.runner != self.runner {
            return Err(invalid("Job catalog belongs to another installation"));
        }
        for job in &mut catalog.jobs {
            for run in &mut job.runs {
                if run.phase == JobPhase::Running {
                    run.phase = JobPhase::Interrupted;
                    run.error = Some(
                        "Interrupted run requires inspection before another invocation".into(),
                    );
                    job.configuration.enabled = false;
                }
            }
        }
        let bytes = store.write(&catalog, Some(&bytes))?;
        Ok(Stored {
            store,
            enabled: Arc::new(AtomicBool::new(
                catalog.trusted
                    && !catalog.safe_mode
                    && catalog.jobs.iter().any(|job| job.configuration.enabled),
            )),
            catalog,
            bytes,
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct StoredJob {
    pub plugin: String,
    pub descriptor: JobDescriptor,
    pub configuration: JobConfiguration,
    pub state: Value,
    pub last_occurrence: Option<String>,
    pub last_scheduled_at: i64,
    pub runs: Vec<JobRun>,
    pub requested: bool,
    pub transferred_to: Option<String>,
    pub notifications: Vec<String>,
    pub release: Option<JobTransfer>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Catalog {
    pub id: Uuid,
    pub vault: Uuid,
    pub schema: u32,
    pub revision: u64,
    pub runner: String,
    pub trusted: bool,
    pub safe_mode: bool,
    pub jobs: Vec<StoredJob>,
    pub imports: Vec<String>,
}

impl StoredRecord for Catalog {
    const KIND: &'static str = "plugin jobs";
    const MAX_RECORD_BYTES: usize = 4 * 1024 * 1024;
    const MAX_RECORDS: usize = 1;
    fn id(&self) -> Uuid {
        self.id
    }
    fn validate(&self, vault: Uuid, id: Uuid) -> io::Result<()> {
        if self.id != id
            || self.vault != vault
            || self.schema != 1
            || self.jobs.len() > 64
            || self.imports.len() > 256
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid job catalog",
            ));
        }
        let mut identities = std::collections::HashSet::new();
        for job in &self.jobs {
            if job.runs.len() > 16
                || job.notifications.len() > 128
                || serde_json::to_vec(&job.state)?.len() > 65536
                || !identities.insert((&job.plugin, &job.descriptor.id))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid stored job",
                ));
            }
            job.configuration
                .schedule
                .validate()
                .map_err(|message| io::Error::new(io::ErrorKind::InvalidData, message))?;
        }
        Ok(())
    }
}

pub(super) struct Stored {
    pub store: RecordStore<Catalog>,
    pub catalog: Catalog,
    pub bytes: Vec<u8>,
    pub enabled: Arc<AtomicBool>,
}
impl Stored {
    /// Publish a complete replacement before changing the in-memory authority.
    pub fn update(&mut self, change: impl FnOnce(&mut Catalog) -> Result<()>) -> Result<()> {
        let mut next = self.catalog.clone();
        change(&mut next)?;
        next.revision = next
            .revision
            .checked_add(1)
            .filter(|value| *value <= 9_007_199_254_740_991)
            .ok_or_else(|| invalid("Job revision limit reached"))?;
        let bytes = self.store.write(&next, Some(&self.bytes))?;
        self.bytes = bytes;
        self.enabled.store(
            next.trusted
                && !next.safe_mode
                && next.jobs.iter().any(|job| job.configuration.enabled),
            Ordering::Release,
        );
        self.catalog = next;
        Ok(())
    }
}
