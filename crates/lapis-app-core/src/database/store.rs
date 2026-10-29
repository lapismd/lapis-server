use super::{Operation, Output, closed, engine, invalid};
use crate::services::{Error, Result};
use lapis_app_protocol::ErrorCode;
use lapis_app_protocol::database::changes::ChangeSet;
use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    path::Path,
};
use uuid::Uuid;

pub(super) struct Store {
    pub embedding: super::search::embedding::Provider,
    pub connection: turso::Connection,
    _database: turso::Database,
    _lease: File,
    pub poisoned: bool,
    pub committed: VecDeque<ChangeSet>,
}
impl Store {
    pub async fn open(
        directory: &Path,
        vault: Uuid,
        embeddings: crate::embedding::host::NativeEmbeddingHost,
    ) -> Result<Self> {
        if !directory.is_absolute() || vault.is_nil() {
            return Err(invalid(
                "Native database needs an absolute profile and vault identity",
            ));
        }
        std::fs::create_dir_all(directory)?;
        let directory = directory.canonicalize()?;
        let path = directory.join(format!("{vault}.db"));
        if std::fs::symlink_metadata(&path).is_ok_and(|value| value.file_type().is_symlink()) {
            return Err(invalid("Application database cannot be a symlink"));
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let lease = options.open(directory.join(format!("{vault}.lock")))?;
        lease.try_lock().map_err(|_| {
            Error::new(ErrorCode::Busy, "Application database already has an owner")
        })?;
        let database = turso::Builder::new_local(
            path.to_str()
                .ok_or_else(|| invalid("Database profile must be UTF-8"))?,
        )
        .experimental_index_method(true)
        .experimental_attach(false)
        .experimental_vacuum(false)
        .build()
        .await
        .map_err(engine)?;
        let connection = database.connect().map_err(engine)?;
        let mut store = Self {
            embedding: super::search::embedding::Provider::new(embeddings),
            connection,
            _database: database,
            _lease: lease,
            poisoned: false,
            committed: VecDeque::new(),
        };
        store.migrate().await?;
        store.revision().await?;
        let config = store
            .get("search.embeddingProvider")
            .await?
            .filter(|v| !v.is_null())
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| invalid("Invalid stored embedding configuration"))?;
        super::search::embedding::validate(&config)?;
        store.embedding.reset(config);
        Ok(store)
    }
    pub async fn execute(&mut self, operation: Operation) -> Result<Output> {
        if self.poisoned {
            return Err(Error::new(
                ErrorCode::Io,
                "Application database requires recovery after failed rollback",
            ));
        }
        match operation {
            Operation::Migrate => self.migrate().await.map(|_| Output::Done),
            Operation::Get(key) => self.get(&key).await.map(Output::Meta),
            Operation::Scan(query) => self.scan(query).await.map(Output::Page),
            Operation::Transact(transaction) => {
                self.transact(transaction).await.map(Output::Applied)
            }
            Operation::Revision => self.revision().await.map(Output::Revision),
            Operation::SaveNotebook(path, state) => {
                self.save_notebook(path, state).await.map(|_| Output::Done)
            }
            Operation::LoadSnapshot => self.load_snapshot().await.map(Output::Snapshot),
            Operation::SaveSnapshot(snapshot) => {
                self.save_snapshot(snapshot).await.map(|_| Output::Done)
            }
            Operation::Notifications(action) => self.notifications(action).await,
            Operation::GetHistory(path) => self.history(&path).await.map(Output::History),
            Operation::StoreHistory(input) => {
                self.store_history(input).await.map(Output::StoredHistory)
            }
            Operation::Indexed(action) => self.indexed(action).await.map(Output::Indexed),
            Operation::Tasks(action) => self.tasks(action).await.map(Output::Tasks),
            Operation::Memory(action) => self.memory(action).await.map(Output::Memory),
            Operation::Search(action) => self.search(action).await.map(Output::Search),
            Operation::Projection(action) => self.projection(action).await.map(|_| Output::Done),
            Operation::ProjectionRead(action) => self
                .read_projection(action)
                .await
                .map(Output::ProjectionRead),
        }
    }
    pub async fn revision(&self) -> Result<u64> {
        let mut rows = self
            .connection
            .query("SELECT revision FROM app_revision WHERE id=1", ())
            .await
            .map_err(engine)?;
        let revision = rows
            .next()
            .await
            .map_err(engine)?
            .ok_or_else(closed)?
            .get::<i64>(0)
            .map_err(engine)?;
        if !(0..=9_007_199_254_740_991).contains(&revision) {
            return Err(invalid("Invalid application database revision"));
        }
        Ok(revision as u64)
    }
}
