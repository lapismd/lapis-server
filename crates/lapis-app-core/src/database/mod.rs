//! Native high-level AppDatabase operations; SQL and paths never enter requests.
mod actor;
mod authority;
mod call;
mod changes;
mod history_client;
mod history_reads;
mod history_writes;
mod indexed;
mod memory;
mod notifications;
mod projections;
mod reads;
mod saved_state;
mod schema;
mod search;
mod state_client;
mod store;
pub(crate) mod tasks;
mod transactions;

use crate::services::{Error, Result};
use lapis_app_protocol::ErrorCode;
pub use lapis_app_protocol::database::{
    MetaCheck, MetaEntry, MetaMutation, MetaPage, MetaScan, MetaTransaction,
};
use lapis_app_protocol::database::{
    changes::ChangeSet,
    history::{FileHistory, StoreHistory, StoredHistory},
    state::{MetadataSnapshot, NotebookState, NotificationRecord},
};
use serde::Serialize;
use serde_json::Value;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::sync::{Semaphore, broadcast, mpsc, oneshot, watch};
use uuid::Uuid;

const MAX_BYTES: usize = 32 * 1024 * 1024;
type Completion = Option<std::result::Result<(), lapis_app_protocol::ServiceError>>;

pub(crate) struct DatabaseCompletion(watch::Receiver<Completion>);
impl DatabaseCompletion {
    pub(crate) async fn wait(mut self) -> Result<()> {
        loop {
            if let Some(result) = self.0.borrow_and_update().clone() {
                return result.map_err(Error);
            }
            self.0.changed().await.map_err(|_| closed())?;
        }
    }
}

/// Clones observe one worker. Dropping a waiter never cancels accepted work.
#[derive(Clone)]
pub struct Database {
    admission: Arc<Mutex<Option<mpsc::Sender<actor::Envelope>>>>,
    authority: Arc<tokio::sync::RwLock<authority::Authority>>,
    bytes: Arc<Semaphore>,
    finished: watch::Receiver<Completion>,
    changes: Arc<broadcast::Receiver<ChangeSet>>,
    embedding_status: watch::Receiver<
        Option<lapis_app_protocol::database::search::embedding::SearchEmbeddingRuntimeStatus>,
    >,
}
#[derive(Serialize)]
enum Operation {
    Migrate,
    Get(String),
    Scan(MetaScan),
    Transact(MetaTransaction),
    Revision,
    SaveNotebook(String, Option<NotebookState>),
    LoadSnapshot,
    SaveSnapshot(MetadataSnapshot),
    Notifications(notifications::Action),
    GetHistory(String),
    StoreHistory(StoreHistory),
    Indexed(indexed::Action),
    Projection(projections::Action),
    ProjectionRead(projections::ReadAction),
    Tasks(tasks::Action),
    Memory(memory::Action),
    Search(search::Action),
}
enum Output {
    Meta(Option<Value>),
    Page(MetaPage),
    Applied(bool),
    Revision(u64),
    Snapshot(Option<MetadataSnapshot>),
    Notifications(Vec<NotificationRecord>),
    Done,
    History(Option<FileHistory>),
    StoredHistory(StoredHistory),
    Indexed(indexed::Value),
    ProjectionRead(projections::ReadValue),
    Tasks(tasks::Value),
    Memory(memory::Value),
    Search(search::Value),
}
impl Database {
    /// Native installation input only; callers derive this from admitted policy.
    pub async fn open(directory: &Path, vault: Uuid) -> Result<Self> {
        Self::open_with_embeddings(
            directory,
            vault,
            crate::embedding::host::NativeEmbeddingHost::default(),
        )
        .await
    }
    pub async fn open_with_embeddings(
        directory: &Path,
        vault: Uuid,
        embeddings: crate::embedding::host::NativeEmbeddingHost,
    ) -> Result<Self> {
        let store = store::Store::open(directory, vault, embeddings).await?;
        let embedding_status = store.embedding.status.subscribe();
        let (sender, receiver) = mpsc::channel(64);
        let (done, finished) = watch::channel(None);
        let (publisher, changes) = broadcast::channel(128);
        tokio::spawn(actor::run(store, receiver, done, publisher));
        Ok(Self {
            admission: Arc::new(Mutex::new(Some(sender))),
            authority: Arc::default(),
            bytes: Arc::new(Semaphore::new(MAX_BYTES)),
            finished,
            changes: Arc::new(changes),
            embedding_status,
        })
    }
    pub async fn get_meta(&self, key: &str) -> Result<Option<Value>> {
        match self.submit(Operation::Get(key.into())).await? {
            Output::Meta(value) => Ok(value),
            _ => unreachable!("worker output matches operation"),
        }
    }
    pub async fn set_meta(&self, key: &str, value: Option<Value>) -> Result<()> {
        self.transact_meta(MetaTransaction {
            checks: vec![],
            mutations: vec![MetaMutation {
                key: key.into(),
                value,
                delete: false,
            }],
        })
        .await
        .map(|_| ())
    }
    pub async fn scan_meta(&self, query: MetaScan) -> Result<MetaPage> {
        match self.submit(Operation::Scan(query)).await? {
            Output::Page(page) => Ok(page),
            _ => unreachable!("worker output matches operation"),
        }
    }
    pub async fn transact_meta(&self, transaction: MetaTransaction) -> Result<bool> {
        if transaction.checks.len() > 1024 || transaction.mutations.len() > 1024 {
            return Err(invalid(
                "Metadata transaction exceeds 1024 checks or mutations",
            ));
        }
        match self.submit(Operation::Transact(transaction)).await? {
            Output::Applied(applied) => Ok(applied),
            _ => unreachable!("worker output matches operation"),
        }
    }
    pub async fn revision(&self) -> Result<u64> {
        match self.submit(Operation::Revision).await? {
            Output::Revision(value) => Ok(value),
            _ => unreachable!("worker output matches operation"),
        }
    }
    /// A lagged receiver must refresh its affected domains before resuming.
    pub fn subscribe_to_changes(&self) -> broadcast::Receiver<ChangeSet> {
        self.changes.resubscribe()
    }
    async fn submit(&self, operation: Operation) -> Result<Output> {
        let size = encoded(&operation)?.len().saturating_add(1);
        let permit = self
            .bytes
            .clone()
            .try_acquire_many_owned(size as u32)
            .map_err(|_| Error::new(ErrorCode::Busy, "Database admission byte limit is busy"))?;
        let (reply, receiver) = oneshot::channel();
        self.admission
            .lock()
            .map_err(|_| closed())?
            .as_ref()
            .ok_or_else(closed)?
            .try_send(actor::Envelope {
                operation,
                reply,
                _permit: permit,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => {
                    Error::new(ErrorCode::Busy, "Database admission queue is full")
                }
                mpsc::error::TrySendError::Closed(_) => closed(),
            })?;
        receiver.await.map_err(|_| {
            Error::new(
                ErrorCode::Io,
                "Database outcome is unavailable; do not retry uncertain writes",
            )
        })?
    }
    /// Fence public admission, drain accepted operations/facades, then checkpoint.
    pub async fn close(&self) -> Result<()> {
        self.fence();
        self.completion().wait().await
    }
    pub(crate) fn fence(&self) {
        self.admission
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
    }
    pub(crate) fn completion(&self) -> DatabaseCompletion {
        DatabaseCompletion(self.finished.clone())
    }
}
fn closed() -> Error {
    Error::new(ErrorCode::Closed, "Application database is closed")
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InvalidRequest, message)
}
fn engine(_: turso::Error) -> Error {
    Error::new(ErrorCode::Io, "Application database operation failed")
}
fn encoded(value: &impl Serialize) -> Result<Vec<u8>> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| invalid("Invalid application database JSON"))?;
    if bytes.len() >= MAX_BYTES {
        return Err(invalid("Application database message exceeds 32 MiB"));
    }
    Ok(bytes)
}
