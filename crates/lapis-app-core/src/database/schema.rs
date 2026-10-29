use super::{Database, Operation, engine, store::Store};
use crate::services::{Error, Result};
use lapis_app_protocol::ErrorCode;

// The normalized on-disk contract is the retained Turso provider's v5 schema.
const VERSION: &str = "5";
const SCHEMAS: [&str; 6] = [
    include_str!("schema.sql"),
    include_str!("indexed/schema.sql"),
    include_str!("projections/schema.sql"),
    include_str!("tasks/schema.sql"),
    include_str!("memory/schema.sql"),
    include_str!("search/schema.sql"),
];

impl Database {
    /// Idempotent current-format initialization; never imports legacy databases.
    pub async fn migrate(&self) -> Result<()> {
        self.submit(Operation::Migrate).await.map(|_| ())
    }
}

impl Store {
    pub async fn migrate(&mut self) -> Result<()> {
        self.begin().await?;
        let result = self.initialize_schema().await.map(|_| ((), None));
        self.finish(result).await
    }

    async fn initialize_schema(&self) -> Result<()> {
        self.check_schema_version().await?;
        for schema in SCHEMAS {
            self.connection
                .execute_batch(schema)
                .await
                .map_err(engine)?;
        }
        self.connection.execute_batch("CREATE INDEX IF NOT EXISTS search_docs_fts ON search_docs USING fts (name,path,content,tags,metadata_text) WITH (weights = 'name=2.0,path=1.5,content=1.0,tags=1.5,metadata_text=1.0')").await.map_err(engine)?;
        self.connection
            .execute(
                "INSERT INTO schema_meta(key,value) VALUES('schema.version',?) \
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [VERSION],
            )
            .await
            .map_err(engine)?;
        self.connection
            .execute(
                "INSERT INTO app_meta(key,value_json) VALUES('schema.version',?) \
                 ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json",
                [VERSION],
            )
            .await
            .map_err(engine)?;
        Ok(())
    }

    async fn check_schema_version(&self) -> Result<()> {
        let mut tables = self
            .connection
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='schema_meta'",
                (),
            )
            .await
            .map_err(engine)?;
        if tables.next().await.map_err(engine)?.is_none() {
            return Ok(());
        }
        let mut rows = self
            .connection
            .query(
                "SELECT value FROM schema_meta WHERE key='schema.version'",
                (),
            )
            .await
            .map_err(engine)?;
        if let Some(row) = rows.next().await.map_err(engine)?
            && row.get::<String>(0).map_err(engine)? != VERSION
        {
            return Err(Error::new(
                ErrorCode::Unsupported,
                "Unsupported application database schema; existing storage was preserved",
            ));
        }
        Ok(())
    }
}
