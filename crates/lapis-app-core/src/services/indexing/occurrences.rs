use crate::{
    database::Database,
    services::{Error, Result},
};
use lapis_app_protocol::{ErrorCode, database::projections::*};
use lapis_task_services::{
    TASK_OCCURRENCE_PROJECTION_VERSION, TaskOccurrenceRecord, is_canonical_daily_document,
    task_occurrence_projection_rows,
};
use serde_json::Value;
use std::collections::HashSet;

const ID: &str = "tasks/occurrence";
const CONFIG: &str = "native-tasks-occurrence-v1";

pub(super) struct Prepared {
    canonical: bool,
    rows: Vec<TaskOccurrenceRecord>,
}
pub(super) fn prepare(path: &str, text: &str) -> Prepared {
    Prepared {
        canonical: is_canonical_daily_document(text, None),
        rows: task_occurrence_projection_rows(text, path),
    }
}
fn definition() -> ProjectionDefinition {
    use ProjectionFieldType::*;
    let fields = [
        ("observationId", String, false, true),
        ("taskId", String, true, false),
        ("taskPath", Path, false, false),
        ("dailyDocumentPath", Path, false, false),
        ("occurrenceDate", Date, true, false),
        ("outcome", String, true, false),
        ("trackingKind", String, false, false),
        ("value", Number, true, false),
        ("unit", String, false, false),
        ("durationMinutes", Number, true, false),
        ("sourceStart", Number, false, false),
        ("sourceEnd", Number, false, false),
    ]
    .into_iter()
    .map(|(name, r#type, sortable, unique)| {
        (
            name.into(),
            ProjectionField {
                r#type,
                indexed: !matches!(name, "sourceStart" | "sourceEnd"),
                sortable,
                unique,
                cardinality: None,
            },
        )
    })
    .collect();
    ProjectionDefinition {
        projection_id: ID.into(),
        owner_plugin_id: "tasks".into(),
        schema_version: TASK_OCCURRENCE_PROJECTION_VERSION.into(),
        config_hash: CONFIG.into(),
        visibility: ProjectionVisibility::Public,
        fields,
        active: true,
        updated_at: 0.0,
    }
}
fn identity(definition: &ProjectionDefinition) -> Result<Value> {
    let mut value =
        serde_json::to_value(definition).map_err(|_| invalid("Invalid occurrence schema"))?;
    value
        .as_object_mut()
        .expect("definition is an object")
        .remove("updatedAt");
    Ok(value)
}
pub(super) async fn register(database: &Database) -> Result<()> {
    let expected = definition();
    let existing = database.native_projection_state(ID, "").await?.definition;
    if let Some(existing) = existing
        && identity(&existing)? == identity(&expected)?
    {
        return Ok(());
    }
    database.register_projection_definition(expected).await
}
pub(super) async fn publish(
    database: &Database,
    path: &str,
    hash: &str,
    prepared: Prepared,
) -> Result<()> {
    let state = database.native_projection_state(ID, path).await?.source;
    if !prepared.canonical {
        if state.is_some() {
            database
                .delete_projection_source(ID, path, Some("tasks"))
                .await?;
        }
        return Ok(());
    }
    if let Some(state) = state
        && state.hash == hash
        && state.schema_version == f64::from(TASK_OCCURRENCE_PROJECTION_VERSION)
        && state.config_hash == CONFIG
    {
        if state.status == "ready" {
            return Ok(());
        }
        if state.status == "error"
            && let Some(error) = state.error
        {
            return Err(invalid(&error));
        }
    }
    let mut ids = HashSet::new();
    let error = if prepared.rows.len() > 64 {
        Some("Tasks occurrence source exceeds 64 rows")
    } else if prepared
        .rows
        .iter()
        .any(|row| !ids.insert(&row.observation_id))
    {
        Some("Duplicate Tasks occurrence identity in source")
    } else {
        None
    };
    if let Some(error) = error {
        database
            .mark_projection_source_error(ProjectionSourceError {
                projection_id: ID.into(),
                source_path: path.into(),
                source_hash: hash.into(),
                writer_plugin_id: Some("tasks".into()),
                error: error.into(),
            })
            .await?;
        return Err(invalid(error));
    }
    let rows = prepared
        .rows
        .into_iter()
        .enumerate()
        .map(|(ordinal, record)| {
            let id = record.observation_id.clone();
            let Value::Object(data) =
                serde_json::to_value(record).map_err(|_| invalid("Invalid occurrence record"))?
            else {
                return Err(invalid("Invalid occurrence record"));
            };
            Ok(ProjectionRow {
                id,
                kind: "observation".into(),
                ordinal: Some(ordinal as f64),
                data,
            })
        })
        .collect::<Result<_>>()?;
    database
        .replace_projection_source(ProjectionSource {
            projection_id: ID.into(),
            source_path: path.into(),
            source_hash: hash.into(),
            writer_plugin_id: Some("tasks".into()),
            rows,
            edges: vec![],
        })
        .await
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InvalidRequest, message)
}
