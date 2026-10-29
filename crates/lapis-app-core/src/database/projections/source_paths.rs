use super::{ReadAction, ReadValue};
use crate::database::{Database, MAX_BYTES, Operation, Output, engine, invalid, store::Store};
use crate::services::Result;

impl Database {
    /// Native owner-only source manifest; ready and error sources share the same cursor.
    pub(crate) async fn native_projection_source_paths(
        &self,
        id: &str,
        after: Option<&str>,
    ) -> Result<Vec<String>> {
        match self
            .submit(Operation::ProjectionRead(ReadAction::NativeSourcePaths(
                id.into(),
                after.map(str::to_owned),
            )))
            .await?
        {
            Output::ProjectionRead(ReadValue::NativeSourcePaths(paths)) => Ok(paths),
            _ => unreachable!("worker output matches operation"),
        }
    }
}

impl Store {
    pub(super) async fn native_projection_source_paths(
        &self,
        id: &str,
        after: Option<&str>,
    ) -> Result<Vec<String>> {
        let mut rows = self.connection.query(
            "SELECT source_path FROM index_projection_sources WHERE projection_id=? AND (? IS NULL OR source_path>?) ORDER BY source_path LIMIT 500",
            turso::params![id, after, after],
        ).await.map_err(engine)?;
        let mut paths = Vec::new();
        let mut bytes = 0usize;
        while let Some(row) = rows.next().await.map_err(engine)? {
            let path: String = row.get(0).map_err(engine)?;
            bytes = bytes.saturating_add(path.len());
            if bytes >= MAX_BYTES {
                return Err(invalid("Native projection source page exceeds 32 MiB"));
            }
            paths.push(path);
        }
        Ok(paths)
    }
}
