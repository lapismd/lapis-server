use crate::{
    database::Database,
    services::{Error, Result},
};
use lapis_app_protocol::{ErrorCode, database::projections::*};
use lapis_plugin_services::markdown::content_hash;

pub(super) const ID: &str = "docs/note";

pub(super) fn root(value: &str) -> String {
    lapis_docs_projection::registration(&serde_json::json!(value))["source"]["pathPrefix"]
        .as_str()
        .expect("qualified Docs registration contains a root")
        .into()
}

pub(super) fn definition(root: &str) -> Result<ProjectionDefinition> {
    let registration = lapis_docs_projection::registration(&serde_json::json!(root));
    Ok(ProjectionDefinition {
        projection_id: ID.into(),
        owner_plugin_id: "docs".into(),
        schema_version: registration["version"]
            .as_f64()
            .expect("qualified Docs version"),
        config_hash: format!("native-docs-note-v1:{}", content_hash(root)),
        visibility: ProjectionVisibility::Public,
        fields: serde_json::from_value(registration["fields"].clone())
            .map_err(|_| Error::new(ErrorCode::InvalidRequest, "Invalid native Docs schema"))?,
        active: true,
        updated_at: 0.0,
    })
}

pub(super) async fn register(database: &Database, definition: &ProjectionDefinition) -> Result<()> {
    let expected = serde_json::to_value(definition).expect("Docs schema serializes");
    if let Some(existing) = database.native_projection_state(ID, "").await?.definition {
        let mut actual = serde_json::to_value(existing).expect("stored schema serializes");
        actual["updatedAt"] = expected["updatedAt"].clone();
        if actual == expected {
            return Ok(());
        }
    }
    database
        .register_projection_definition(definition.clone())
        .await
}

pub(super) fn matches(path: &str, root: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".doc.md")
        && (path == root
            || path
                .strip_prefix(root)
                .is_some_and(|tail| tail.starts_with('/')))
        && !path.split('/').any(|part| part.starts_with('.'))
}
