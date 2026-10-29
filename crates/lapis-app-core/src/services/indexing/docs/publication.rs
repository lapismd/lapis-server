use super::definition::ID;
use crate::{
    database::Database,
    services::{Error, Result},
};
use lapis_app_protocol::{ErrorCode, database::projections::*};

pub(super) async fn publish(
    database: &Database,
    definition: &ProjectionDefinition,
    path: &str,
    hash: &str,
    result: std::result::Result<serde_json::Value, lapis_docs_projection::ProjectionError>,
) -> Result<()> {
    let state = database.native_projection_state(ID, path).await?.source;
    let current = state.as_ref().is_some_and(|state| {
        state.hash == hash
            && state.schema_version == definition.schema_version
            && state.config_hash == definition.config_hash
    });
    match result {
        Ok(result) => {
            let rows: Vec<ProjectionRow> = serde_json::from_value(result["rows"].clone())
                .map_err(|_| Error::new(ErrorCode::InvalidRequest, "Invalid native Docs rows"))?;
            if current
                && state.as_ref().is_some_and(|state| state.status == "ready")
                && database.get_projection_row(ID, path, None).await?
                    == rows
                        .first()
                        .map(|row| serde_json::Value::Object(row.data.clone()))
            {
                return Ok(());
            }
            database
                .replace_projection_source(ProjectionSource {
                    projection_id: ID.into(),
                    source_path: path.into(),
                    source_hash: hash.into(),
                    rows,
                    edges: vec![],
                    writer_plugin_id: Some("docs".into()),
                })
                .await
        }
        Err(error) => {
            if current
                && state.as_ref().is_some_and(|state| {
                    state.status == "error"
                        && state.error.as_deref() == Some(error.message.as_str())
                })
            {
                return Ok(());
            }
            database
                .mark_projection_source_error(ProjectionSourceError {
                    projection_id: ID.into(),
                    source_path: path.into(),
                    source_hash: hash.into(),
                    error: error.message,
                    writer_plugin_id: Some("docs".into()),
                })
                .await
        }
    }
}
