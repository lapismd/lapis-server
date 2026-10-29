use lapis_app_core::database::Database;
use serde_json::{Value, json};
use uuid::Uuid;
mod indexed_support;
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/task-projection.json"
    ))
    .unwrap()
}
async fn seed(db: &Database) {
    for row in fixture()["records"].as_array().unwrap() {
        db.upsert_task_projection(serde_json::from_value(row.clone()).unwrap())
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn task_descendants_follow_only_resolved_task_edges_in_breadth_first_order_and_deduplicate_cycles()
 {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    seed(&db).await;
    for (source, edges) in [
        (
            "full",
            json!([
                {"sourceRowId":"full","relation":"task-entry","targetRowId":"inbox","ordinal":0},
                {"sourceRowId":"full","relation":"list-entry","targetPath":"tasks/list.md","ordinal":1},
                {"sourceRowId":"full","relation":"task-entry","targetRowId":"inbox","ordinal":2},
                {"sourceRowId":"full","relation":"reference","targetRowId":"completed","ordinal":3},
                {"sourceRowId":"full","relation":"task-entry","targetRowId":"missing","ordinal":4}
            ]),
        ),
        (
            "inbox",
            json!([{"sourceRowId":"inbox","relation":"task-entry","targetRowId":"future","ordinal":0}]),
        ),
        (
            "future",
            json!([{"sourceRowId":"future","relation":"list-entry","targetRowId":"full","ordinal":0}]),
        ),
    ] {
        let row = fixture()["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["documentId"] == source)
            .unwrap()
            .clone();
        db.replace_projection_source(serde_json::from_value(json!({"projectionId":"tasks/task","sourcePath":row["documentPath"],"sourceHash":source,"rows":[{"id":source,"kind":row["kind"],"data":row}],"edges":edges})).unwrap()).await.unwrap();
    }
    let revision = db.revision().await.unwrap();
    let rows = db.list_task_descendants("tasks/full.md").await.unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r["documentId"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["inbox", "list", "future"]
    );
    assert!(
        db.list_task_descendants("missing.md")
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(db.revision().await.unwrap(), revision);
    db.delete_task_projection("tasks/inbox.md").await.unwrap();
    assert_eq!(
        db.list_task_descendants("tasks/full.md")
            .await
            .unwrap()
            .iter()
            .map(|r| r["documentId"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["list"]
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn child_links_preserve_normalized_order_kind_and_source_filters_without_the_generic_page_limit()
 {
    use lapis_app_protocol::database::indexed::records::LinkKind;
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let mut input = indexed_support::record("parent.md", 1);
    let template = input.links[0].clone();
    input.links = (0..1001)
        .map(|n| {
            let mut link = template.clone();
            link.target_text = format!("child-{n}");
            link.ordinal = Some((1000 - n) as f64);
            link.kind = Some(if n % 2 == 0 {
                LinkKind::TaskEntry
            } else {
                LinkKind::Reference
            });
            link
        })
        .collect();
    db.upsert_indexed_metadata(input).await.unwrap();
    let query = json!({"sourcePath":"parent.md"});
    let links = db
        .list_child_links(serde_json::from_value(query.clone()).unwrap())
        .await
        .unwrap();
    assert_eq!(links.len(), 1001);
    // The retained Turso provider orders by normalized array position; payload
    // ordinals remain intact rather than becoming a second ordering authority.
    assert_eq!(links[0].target_text, "child-0");
    assert_eq!(links[0].ordinal, Some(1000.0));
    assert_eq!(links[1000].target_text, "child-1000");
    assert_eq!(links[1000].ordinal, Some(0.0));
    let mut filtered = query;
    filtered["kind"] = json!("task-entry");
    assert_eq!(
        db.list_child_links(serde_json::from_value(filtered).unwrap())
            .await
            .unwrap()
            .len(),
        501
    );
    assert!(
        db.list_child_links(serde_json::from_value(json!({"sourcePath":"other.md"})).unwrap())
            .await
            .unwrap()
            .is_empty()
    );
    db.close().await.unwrap();
}
