#![allow(dead_code)]
use lapis_app_core::{database::Database, services::Result};
use serde_json::{Value, json};

pub fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../packages/api/src/lib/__tests__/fixtures/indexed-mutations.json"
    ))
    .unwrap()
}
pub async fn seed(db: &Database, seed: &Value) {
    for row in seed["indexed"].as_array().unwrap() {
        db.upsert_indexed_file(serde_json::from_value(row.clone()).unwrap())
            .await
            .unwrap();
    }
    let state = &seed["notebook"];
    db.set_notebook_state(
        state["sourcePath"].as_str().unwrap(),
        serde_json::from_value(state.clone()).unwrap(),
    )
    .await
    .unwrap();
    db.upsert_search_document(serde_json::from_value(seed["search"].clone()).unwrap())
        .await
        .unwrap();
    db.register_projection_definition(serde_json::from_value(seed["definition"].clone()).unwrap())
        .await
        .unwrap();
    for row in seed["sources"].as_array().unwrap() {
        db.replace_projection_source(serde_json::from_value(row.clone()).unwrap())
            .await
            .unwrap();
    }
}
pub async fn snapshot(db: &Database) -> Value {
    let mut paths = vec![];
    for path in fixture()["paths"].as_array().unwrap() {
        let path = path.as_str().unwrap();
        paths.push(json!({
            "path":path,
            "indexed":db.get_indexed_file(path).await.unwrap(),
            "notebook":db.get_notebook_state(path).await.unwrap(),
            "task":db.get_task_row(serde_json::from_value(json!({"path":path})).unwrap()).await.unwrap(),
            "search":db.get_search_document(path).await.unwrap(),
            "incoming":db.query_metadata_links(serde_json::from_value(json!({"direction":"incoming","path":path})).unwrap()).await.unwrap(),
        }));
    }
    json!({
        "paths":paths,
        "manifest":db.list_indexed_file_manifest(Default::default()).await.unwrap(),
        "searchManifest":db.list_search_document_manifest(Default::default()).await.unwrap(),
        "searchStats":db.get_search_index_stats().await.unwrap(),
        "projection":db.query_projection("fixture/items",serde_json::from_value(json!({"orderBy":[{"field":"id"}]})).unwrap(),None).await.unwrap(),
        "stale":db.query_projection("fixture/items",serde_json::from_value(json!({"includeStale":true,"orderBy":[{"field":"id"}]})).unwrap(),None).await.unwrap(),
        "related":db.query_related(serde_json::from_value(json!({"projectionId":"fixture/items","rowId":"ref","relation":"reference"})).unwrap(),None).await.unwrap(),
        "revision":db.revision().await.unwrap(),
    })
}
pub async fn apply(db: &Database, step: &Value) -> Result<()> {
    match step["kind"].as_str().unwrap() {
        "upsert" => {
            db.upsert_indexed_file(serde_json::from_value(step["record"].clone()).unwrap())
                .await
        }
        "rename" => {
            db.rename_indexed_file(
                step["oldPath"].as_str().unwrap(),
                step["newPath"].as_str().unwrap(),
            )
            .await
        }
        "delete" => db.delete_indexed_file(step["path"].as_str().unwrap()).await,
        _ => panic!("unknown fixture operation"),
    }
}
pub async fn run(db: &Database, steps: &[Value]) -> Value {
    let before = db.revision().await.unwrap();
    let mut events = db.subscribe_to_changes();
    let mut states = vec![];
    for step in steps {
        let failed = apply(db, step).await.is_err();
        states.push(json!({"failed":failed,"state":snapshot(db).await}));
    }
    let mut changes = vec![];
    while let Ok(event) = events.try_recv() {
        changes.push(json!({"revision":event.revision-before,"domains":event.domains,"paths":event.paths,"renamed":event.renamed}));
    }
    json!({"events":changes,"states":states})
}
