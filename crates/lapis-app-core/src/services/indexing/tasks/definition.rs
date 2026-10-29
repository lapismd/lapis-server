use crate::{
    database::{Database, tasks::NATIVE_CONFIG},
    services::{Error, Result},
};
use lapis_app_protocol::{ErrorCode, database::projections::*};
use lapis_task_services::{TASK_PROJECTION_VERSION, TASKS_PUBLIC_PROJECTION_ID};

pub(super) async fn register(database: &Database) -> Result<()> {
    let expected = ProjectionDefinition {
        projection_id: TASKS_PUBLIC_PROJECTION_ID.into(),
        owner_plugin_id: "tasks".into(),
        schema_version: TASK_PROJECTION_VERSION.into(),
        config_hash: NATIVE_CONFIG.into(),
        visibility: ProjectionVisibility::Public,
        fields: serde_json::from_str(include_str!("../../../database/tasks/fields.json"))
            .map_err(|_| Error::new(ErrorCode::InvalidRequest, "Invalid native Task schema"))?,
        active: true,
        updated_at: 0.0,
    };
    let existing = database
        .native_projection_state(TASKS_PUBLIC_PROJECTION_ID, "")
        .await?
        .definition;
    if let Some(existing) = existing {
        let mut actual = serde_json::to_value(existing).expect("schema serializes");
        let mut expected_value = serde_json::to_value(&expected).expect("schema serializes");
        actual.as_object_mut().unwrap().remove("updatedAt");
        expected_value.as_object_mut().unwrap().remove("updatedAt");
        if actual == expected_value {
            return Ok(());
        }
    }
    database.register_projection_definition(expected).await
}
