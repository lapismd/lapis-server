use super::{NativeProjectionState, NativeSourceState, ReadAction, ReadValue};
use crate::database::{MAX_BYTES, engine, invalid, store::Store};
use crate::services::{Error, Result};
use lapis_app_protocol::{
    ErrorCode,
    database::projections::{ProjectionDefinition, ProjectionVisibility},
};

impl Store {
    pub async fn read_projection(&self, action: ReadAction) -> Result<ReadValue> {
        match action {
            ReadAction::NativeSourcePaths(id, after) => self
                .native_projection_source_paths(&id, after.as_deref())
                .await
                .map(ReadValue::NativeSourcePaths),
            ReadAction::NativeState(id, path) => self
                .native_projection_state(&id, &path)
                .await
                .map(ReadValue::NativeState),
            ReadAction::Get(id, row, reader) => self
                .projection_row(&id, &row, reader.as_deref())
                .await
                .map(ReadValue::Row),
            ReadAction::Query(id, query, reader) => self
                .query_projection(&id, query, reader.as_deref())
                .await
                .map(ReadValue::Query),
            ReadAction::Related(query, reader) => self
                .query_related(query, reader.as_deref())
                .await
                .map(ReadValue::Query),
        }
    }
    async fn native_projection_state(&self, id: &str, path: &str) -> Result<NativeProjectionState> {
        let mut rows = self
            .connection
            .query(
                "SELECT active FROM index_projections WHERE projection_id=?",
                [id],
            )
            .await
            .map_err(engine)?;
        let active = match rows.next().await.map_err(engine)? {
            Some(row) => row.get::<i64>(0).map_err(engine)? != 0,
            None => false,
        };
        drop(rows);
        let definition = if active {
            Some(self.projection_definition(id).await?)
        } else {
            None
        };
        let mut rows = self.connection.query(
            "SELECT source_hash,schema_version,config_hash,status,error FROM index_projection_sources WHERE projection_id=? AND source_path=?",
            turso::params![id, path]
        ).await.map_err(engine)?;
        let source = match rows.next().await.map_err(engine)? {
            Some(row) => Some(NativeSourceState {
                hash: row.get(0).map_err(engine)?,
                schema_version: super::definitions::number(&row, 1)?,
                config_hash: row.get(2).map_err(engine)?,
                status: row.get(3).map_err(engine)?,
                error: crate::database::indexed::optional_text(&row, 4)?,
            }),
            None => None,
        };
        Ok(NativeProjectionState { definition, source })
    }
    pub(in crate::database) async fn projection_row(
        &self,
        id: &str,
        row_id: &str,
        reader: Option<&str>,
    ) -> Result<Option<serde_json::Value>> {
        let definition = self.readable_projection(id, reader).await?;
        let mut rows=self.connection.query("SELECT r.data_json FROM index_projection_rows r INNER JOIN index_projection_sources s ON s.projection_id=r.projection_id AND s.source_path=r.source_path LEFT JOIN files f ON f.path=r.source_path WHERE r.projection_id=? AND (r.row_id=? OR r.source_path=? OR json_extract(r.data_json,'$.documentId')=? OR json_extract(r.data_json,'$.id')=?) AND s.status='ready' AND s.schema_version=? AND s.config_hash=? AND (f.path IS NULL OR (f.hash=s.source_hash AND f.deleted=0)) LIMIT 1",turso::params![id,row_id,row_id,row_id,row_id,definition.schema_version,definition.config_hash]).await.map_err(engine)?;
        let Some(row) = rows.next().await.map_err(engine)? else {
            return Ok(None);
        };
        let data = row.get::<String>(0).map_err(engine)?;
        if data.len() >= MAX_BYTES {
            return Err(invalid("Projection row exceeds 32 MiB"));
        }
        // Match the retained provider's explicit parseJson(data, null) boundary.
        Ok(serde_json::from_str(&data)
            .ok()
            .filter(|v| !matches!(v, serde_json::Value::Null)))
    }
    pub(super) async fn readable_projection(
        &self,
        id: &str,
        reader: Option<&str>,
    ) -> Result<ProjectionDefinition> {
        let definition = self.projection_definition(id).await?;
        if matches!(definition.visibility, ProjectionVisibility::Private)
            && reader.filter(|r| !r.is_empty()) != Some(definition.owner_plugin_id.as_str())
        {
            return Err(Error::new(
                ErrorCode::PermissionDenied,
                "Projection is private",
            ));
        }
        Ok(definition)
    }
}
