use lapis_app_core::database::Database;
use serde_json::{Value, json};
use std::{future::Future, task::Poll};
use uuid::Uuid;
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/task-projection.json"
    ))
    .unwrap()
}

#[tokio::test]
async fn native_task_records_and_views_match_the_retained_reference_fixture() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    let mut events = db.subscribe_to_changes();
    for row in fixture()["records"].as_array().unwrap() {
        db.upsert_task_projection(serde_json::from_value(row.clone()).unwrap())
            .await
            .unwrap();
    }
    let revision = db.revision().await.unwrap();
    let observed = std::iter::from_fn(|| events.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(observed.len() as u64, revision);
    assert_eq!(
        observed.iter().map(|e| e.revision).collect::<Vec<_>>(),
        (1..=revision).collect::<Vec<_>>()
    );
    for case in fixture()["cases"].as_array().unwrap() {
        let rows = db
            .query_tasks(serde_json::from_value(case["query"].clone()).unwrap())
            .await
            .unwrap();
        assert_eq!(
            rows.iter()
                .map(|r| r["documentId"].clone())
                .collect::<Vec<_>>(),
            *case["ids"].as_array().unwrap(),
            "{}",
            case["query"]
        );
    }
    let full = fixture()["records"][0].clone();
    assert_eq!(
        db.get_task_row(serde_json::from_value(json!({"id":"full"})).unwrap())
            .await
            .unwrap(),
        Some(full.clone())
    );
    assert_eq!(
        db.get_task_row(serde_json::from_value(json!({"path":"tasks/full.md"})).unwrap())
            .await
            .unwrap(),
        Some(full)
    );
    assert_eq!(db.revision().await.unwrap(), revision);
    db.close().await.unwrap();
    let db = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(
        db.query_tasks(
            serde_json::from_value(json!({"view":"review","today":"2026-10-24"})).unwrap()
        )
        .await
        .unwrap()
        .len(),
        2
    );
    db.delete_task_projection("tasks/full.md").await.unwrap();
    assert!(
        db.get_task_row(serde_json::from_value(json!({"path":"tasks/full.md"})).unwrap())
            .await
            .unwrap()
            .is_none()
    );
    let event = db.subscribe_to_changes();
    drop(event);
    db.close().await.unwrap();
}

#[tokio::test]
async fn one_accepted_task_write_drains_all_commit_stages_after_waiter_cancellation() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    let mut events = db.subscribe_to_changes();
    let mut write = Box::pin(
        db.upsert_task_projection(serde_json::from_value(fixture()["records"][0].clone()).unwrap()),
    );
    std::future::poll_fn(|cx| {
        assert!(write.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(write);
    db.close().await.unwrap();
    let changes = std::iter::from_fn(|| events.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(changes.len(), 3);
    assert_eq!(
        changes
            .iter()
            .map(|e| serde_json::to_value(&e.domains).unwrap())
            .collect::<Vec<_>>(),
        vec![
            json!(["projection"]),
            json!(["task"]),
            json!(["projection"])
        ]
    );
    let db = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(db.query_tasks(Default::default()).await.unwrap().len(), 1);
    db.close().await.unwrap();
}
