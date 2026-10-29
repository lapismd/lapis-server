use lapis_app_core::database::Database;
use lapis_app_protocol::{ErrorCode, database::tasks::record::TaskRecord};
use serde_json::{Value, json};
use uuid::Uuid;
mod indexed_support;
mod projection_support;
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/task-projection.json"
    ))
    .unwrap()
}

#[tokio::test]
async fn failed_projection_stage_retains_prior_task_and_metadata_commits_and_only_publishes_committed_revisions()
 {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let full = fixture()["records"][0].clone();
    db.upsert_task_projection(serde_json::from_value(full.clone()).unwrap())
        .await
        .unwrap();
    db.upsert_indexed_metadata(indexed_support::record("other.md", 2))
        .await
        .unwrap();
    let before = db.revision().await.unwrap();
    let mut events = db.subscribe_to_changes();
    let mut other = full.clone();
    other["documentPath"] = json!("other.md");
    other["title"] = json!("Retained task stage");
    // The existing full row owns this ID; inserting it for another source fails.
    assert_eq!(
        db.upsert_task_projection(serde_json::from_value(other.clone()).unwrap())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Io
    );
    assert_eq!(db.revision().await.unwrap(), before + 1);
    let event = events.try_recv().unwrap();
    assert_eq!(event.revision, before + 1);
    assert_eq!(
        serde_json::to_value(event.domains).unwrap(),
        json!(["task"])
    );
    assert_eq!(event.paths, ["other.md"]);
    assert!(events.try_recv().is_err());
    assert_eq!(
        db.get_task_row(serde_json::from_value(json!({"path":"other.md"})).unwrap())
            .await
            .unwrap(),
        Some(other)
    );
    assert_eq!(
        db.get_task_row(serde_json::from_value(json!({"id":"full"})).unwrap())
            .await
            .unwrap(),
        Some(full)
    );
    assert!(db.get_indexed_file("other.md").await.unwrap().is_some());
    assert_eq!(db.query_tasks(Default::default()).await.unwrap().len(), 1);
    db.set_meta("after-failure", Some(json!(true)))
        .await
        .unwrap();
    db.close().await.unwrap();
}

#[tokio::test]
async fn task_schema_and_atomic_deletion_keep_the_retained_fields_and_domain_change() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    db.upsert_task_projection(serde_json::from_value(fixture()["records"][0].clone()).unwrap())
        .await
        .unwrap();
    db.close().await.unwrap();
    let stored = projection_support::inspect(
        profile.path(),
        id,
        "SELECT fields_json FROM index_projections WHERE projection_id='tasks/task'",
    )
    .await;
    let fields: Value = serde_json::from_str(stored[0][0].as_str().unwrap()).unwrap();
    let mut expected = fixture()["fields"].clone();
    // Optional source flags mean false. Native DTOs serialize those defaults
    // explicitly; compare their meaning as well as the complete field set.
    for field in expected.as_object_mut().unwrap().values_mut() {
        for flag in ["indexed", "sortable", "unique"] {
            field
                .as_object_mut()
                .unwrap()
                .entry(flag)
                .or_insert(json!(false));
        }
    }
    assert_eq!(fields, expected);
    let db = Database::open(profile.path(), id).await.unwrap();
    let before = db.revision().await.unwrap();
    let mut events = db.subscribe_to_changes();
    db.delete_task_projection("tasks/full.md").await.unwrap();
    let event = events.try_recv().unwrap();
    assert_eq!(event.revision, before + 1);
    assert_eq!(
        serde_json::to_value(event.domains).unwrap(),
        json!(["task", "projection"])
    );
    assert!(events.try_recv().is_err());
    assert!(
        db.get_task_row(serde_json::from_value(json!({"id":"full"})).unwrap())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        db.get_task_row(serde_json::from_value(json!({"path":"tasks/full.md"})).unwrap())
            .await
            .unwrap()
            .is_none()
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn invalid_nested_task_numbers_reject_before_definition_or_record_admission() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let mut events = db.subscribe_to_changes();
    let base: TaskRecord = serde_json::from_value(fixture()["records"][0].clone()).unwrap();
    let mut counts = base.clone();
    counts.checklist_total = f64::NAN;
    let mut recurrence = base.clone();
    recurrence
        .repeat
        .as_mut()
        .unwrap()
        .as_mut()
        .unwrap()
        .completion_count = Some(f64::INFINITY);
    let mut structure = base.clone();
    structure
        .structure
        .as_mut()
        .unwrap()
        .as_mut()
        .unwrap()
        .entries[0]
        .heading_level = Some(f64::NAN);
    for row in [counts, recurrence, structure] {
        assert_eq!(
            db.upsert_task_projection(row).await.unwrap_err().code(),
            ErrorCode::InvalidRequest
        );
    }
    assert_eq!(db.revision().await.unwrap(), 0);
    assert!(events.try_recv().is_err());
    db.close().await.unwrap();
}
