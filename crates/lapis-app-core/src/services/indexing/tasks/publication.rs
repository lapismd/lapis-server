use crate::{
    database::{Database, tasks::NATIVE_CONFIG},
    services::{Error, Result},
};
use lapis_app_protocol::{ErrorCode, database::projections::*};
use lapis_plugin_services::markdown::content_hash;
use lapis_task_services::{
    ParsedTaskDocument, ResolvedTaskDocumentStructure, TASK_PROJECTION_VERSION,
    TASKS_PUBLIC_PROJECTION_ID, task_projection_source,
};

pub(super) async fn publish(
    database: &Database,
    doc: &ParsedTaskDocument,
    structure: Option<&ResolvedTaskDocumentStructure>,
    today: &str,
) -> Result<()> {
    let source = task_projection_source(doc, structure, today)
        .map_err(|error| Error::new(ErrorCode::InvalidRequest, &error.message))?;
    let mut source: ProjectionSource = serde_json::from_value(
        serde_json::to_value(source).expect("native Task envelope serializes"),
    )
    .map_err(|_| Error::new(ErrorCode::InvalidRequest, "Invalid native Task envelope"))?;
    // Plugin envelopes preserve compatibility identities; this admitted owner
    // binds freshness to the physical Markdown already published by its core pass.
    source.source_hash = content_hash(&doc.raw);
    let state = database
        .native_projection_state(TASKS_PUBLIC_PROJECTION_ID, &doc.path)
        .await?
        .source;
    if let Some(state) = state
        && state.hash == source.source_hash
        && state.schema_version == f64::from(TASK_PROJECTION_VERSION)
        && state.config_hash == NATIVE_CONFIG
        && state.status == "ready"
        && database
            .get_projection_row(TASKS_PUBLIC_PROJECTION_ID, &doc.path, None)
            .await?
            == Some(serde_json::Value::Object(source.rows[0].data.clone()))
    {
        return Ok(());
    }
    database.replace_projection_source(source).await
}
pub(super) async fn prune(database: &Database, path: &str) -> Result<()> {
    if database
        .native_projection_state(TASKS_PUBLIC_PROJECTION_ID, path)
        .await?
        .source
        .is_some()
    {
        database
            .delete_projection_source(TASKS_PUBLIC_PROJECTION_ID, path, Some("tasks"))
            .await?;
    }
    Ok(())
}
