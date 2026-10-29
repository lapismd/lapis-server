use super::support::*;
use lapis_app_protocol::database::projections::{ProjectionRow, ProjectionSource};
use lapis_plugin_services::markdown::content_hash;
use serde_json::json;
use uuid::Uuid;

const PATH: &str = "List.md";
const CONFIG: &str = "native-tasks-document-v4-yaml12-1";

#[tokio::test]
async fn cold_same_byte_yaml12_intake_replaces_old_private_cache_identity() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let raw = "---\ntype: task-list\nid: list\ntitle: \"Top\u{85}line\"\n---\n\n# List\n";
    write(&root, PATH, raw);
    let id = Uuid::new_v4();
    let first = runtime(&profile);
    let vault = first.admit(id, root.path()).unwrap();
    vault.set_task_document_indexing(true).await.unwrap();
    until(async || row(&vault, PATH).await["title"] == "Top\u{85}line").await;
    vault.finish_indexing().unwrap();
    let database = vault.database().await.unwrap();
    let mut definition = database
        .native_projection_state("tasks/task", PATH)
        .await
        .unwrap()
        .definition
        .unwrap();
    let mut data = database
        .get_projection_row("tasks/task", PATH, None)
        .await
        .unwrap()
        .unwrap();
    data["title"] = json!("Top line");
    definition.config_hash = "native-tasks-document-v4".into();
    database
        .register_projection_definition(definition)
        .await
        .unwrap();
    database
        .replace_projection_source(ProjectionSource {
            projection_id: "tasks/task".into(),
            source_path: PATH.into(),
            source_hash: content_hash(raw),
            rows: vec![ProjectionRow {
                id: "list".into(),
                kind: "task-list".into(),
                ordinal: None,
                data: data.as_object().unwrap().clone(),
            }],
            edges: vec![],
            writer_plugin_id: Some("tasks".into()),
        })
        .await
        .unwrap();
    assert_eq!(
        database
            .get_projection_row("tasks/task", PATH, None)
            .await
            .unwrap()
            .unwrap()["title"],
        "Top line"
    );
    drop(database);
    close(first).await;

    for _ in 0..2 {
        let current = runtime(&profile);
        let vault = current.admit(id, root.path()).unwrap();
        vault.set_task_document_indexing(true).await.unwrap();
        until(async || row(&vault, PATH).await["title"] == "Top\u{85}line").await;
        let database = vault.database().await.unwrap();
        let state = database
            .native_projection_state("tasks/task", PATH)
            .await
            .unwrap();
        assert_eq!(state.definition.unwrap().config_hash, CONFIG);
        let source = state.source.unwrap();
        assert_eq!(source.config_hash, CONFIG);
        assert_eq!(source.hash, content_hash(raw));
        assert_eq!(source.schema_version, 4.0);
        let before = row(&vault, PATH).await;
        rescan(&vault, &root).await;
        assert_eq!(row(&vault, PATH).await, before);
        assert_eq!(
            std::fs::read_to_string(root.path().join(PATH)).unwrap(),
            raw
        );
        drop(database);
        close(current).await;
    }
}
