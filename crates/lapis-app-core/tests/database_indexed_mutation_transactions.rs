use lapis_app_core::database::Database;
use lapis_app_protocol::{ErrorCode, database::indexed::mutations::IndexedFile};
use serde_json::{Value, json};
use std::{future::Future, task::Poll};
use uuid::Uuid;
mod indexed_mutation_support;
mod projection_support;
mod search_support;
use indexed_mutation_support::{apply, fixture, seed, snapshot};
use search_support::assert_json;

#[tokio::test]
async fn accepted_upsert_rename_and_delete_finish_after_their_waiters_detach() {
    for index in [1, 3, 4] {
        let input = fixture();
        let case = &input["cases"][index];
        let profile = tempfile::tempdir().unwrap();
        let id = Uuid::new_v4();
        let db = Database::open(profile.path(), id).await.unwrap();
        seed(&db, &case["seed"]).await;
        let step = &case["steps"][0];
        let mut pending = Box::pin(apply(&db, step));
        std::future::poll_fn(|cx| {
            assert!(pending.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        drop(pending);
        db.close().await.unwrap();
        assert_eq!(
            apply(&db, step).await.unwrap_err().code(),
            ErrorCode::Closed
        );
        let db = Database::open(profile.path(), id).await.unwrap();
        assert_json(
            snapshot(&db).await,
            case["expected"]["states"][0]["state"].clone(),
        );
        db.close().await.unwrap();
    }
}

#[tokio::test]
async fn later_task_failure_keeps_core_and_task_commits_and_only_publishes_completed_stages() {
    let input = fixture();
    let case = &input["cases"][1];
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    seed(&db, &case["seed"]).await;
    let before = db.revision().await.unwrap();
    let mut events = db.subscribe_to_changes();
    let mut row = case["steps"][0]["record"].clone();
    row["file"]["path"] = json!("other.md");
    row["file"]["normalizedPath"] = json!("other.md");
    row["metadata"]["path"] = json!("other.md");
    row["task"]["documentPath"] = json!("other.md");
    for (key, field) in [
        ("links", "sourcePath"),
        ("tags", "path"),
        ("properties", "path"),
    ] {
        for value in row[key].as_array_mut().unwrap() {
            value[field] = json!("other.md");
        }
    }
    assert_eq!(
        db.upsert_indexed_file(serde_json::from_value(row).unwrap())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Io
    );
    assert_eq!(db.revision().await.unwrap(), before + 2);
    assert!(db.get_indexed_file("other.md").await.unwrap().is_some());
    assert!(
        db.get_task_row(serde_json::from_value(json!({"path":"other.md"})).unwrap())
            .await
            .unwrap()
            .is_some()
    );
    for (index, domain) in ["metadata", "task"].iter().enumerate() {
        let event = events.try_recv().unwrap();
        assert_eq!(event.revision, before + index as u64 + 1);
        assert_eq!(
            serde_json::to_value(event.domains).unwrap(),
            json!([domain])
        );
        assert_eq!(event.paths, ["other.md"]);
    }
    assert!(events.try_recv().is_err());
    db.close().await.unwrap();
}

#[tokio::test]
async fn exhausted_revision_rolls_back_the_complete_rename_and_delete() {
    let input = fixture();
    let case = &input["cases"][3];
    for step in [&case["steps"][0], &input["cases"][4]["steps"][0]] {
        let profile = tempfile::tempdir().unwrap();
        let id = Uuid::new_v4();
        let db = Database::open(profile.path(), id).await.unwrap();
        seed(&db, &case["seed"]).await;
        db.close().await.unwrap();
        let raw =
            turso::Builder::new_local(profile.path().join(format!("{id}.db")).to_str().unwrap())
                .build()
                .await
                .unwrap();
        let connection = raw.connect().unwrap();
        connection
            .execute(
                "UPDATE app_revision SET revision=9007199254740991 WHERE id=1",
                (),
            )
            .await
            .unwrap();
        drop(connection);
        drop(raw);
        let db = Database::open(profile.path(), id).await.unwrap();
        let before = snapshot(&db).await;
        let mut events = db.subscribe_to_changes();
        assert_eq!(
            apply(&db, step).await.unwrap_err().code(),
            ErrorCode::InvalidRequest
        );
        assert_json(snapshot(&db).await, before);
        assert!(events.try_recv().is_err());
        db.close().await.unwrap();
    }
}

#[tokio::test]
async fn invalid_task_is_rejected_before_core_admission_and_large_rename_detail_becomes_a_reset() {
    let input = fixture();
    let case = &input["cases"][1];
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    seed(&db, &case["seed"]).await;
    let before = db.revision().await.unwrap();
    let mut events = db.subscribe_to_changes();
    let mut row: IndexedFile = serde_json::from_value(case["steps"][0]["record"].clone()).unwrap();
    row.task.as_mut().unwrap().as_mut().unwrap().checklist_total = f64::NAN;
    assert_eq!(
        db.upsert_indexed_file(row).await.unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(db.revision().await.unwrap(), before);
    assert!(events.try_recv().is_err());
    let new = format!("{}.md", "x".repeat(40 * 1024));
    let old = input["paths"][0].as_str().unwrap();
    db.rename_indexed_file(old, &new).await.unwrap();
    let event = events.try_recv().unwrap();
    assert_eq!(event.reset, Some(true));
    assert!(event.paths.is_empty());
    assert!(event.renamed.is_none());
    assert!(db.get_indexed_file(&new).await.unwrap().is_some());
    db.close().await.unwrap();
    let stored = projection_support::inspect(
        profile.path(),
        id,
        "SELECT paths_json,renamed_json FROM app_changes ORDER BY revision DESC LIMIT 1",
    )
    .await;
    assert_eq!(
        stored,
        vec![vec![
            Value::String("[]".into()),
            Value::String("null".into())
        ]]
    );
}

#[tokio::test]
async fn corrupt_notebook_does_not_panic_or_publish_a_partial_rename() {
    let input = fixture();
    let case = &input["cases"][3];
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    seed(&db, &case["seed"]).await;
    let old = input["paths"][0].as_str().unwrap();
    let key = format!("notebook.state:{old}");
    db.set_meta(&key, Some(json!(5))).await.unwrap();
    let before = db.revision().await.unwrap();
    let mut events = db.subscribe_to_changes();
    assert_eq!(
        apply(&db, &case["steps"][0]).await.unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(db.revision().await.unwrap(), before);
    assert!(events.try_recv().is_err());
    assert_eq!(db.get_meta(&key).await.unwrap(), Some(json!(5)));
    assert!(db.get_indexed_file(old).await.unwrap().is_some());
    assert!(
        db.get_indexed_file(input["paths"][1].as_str().unwrap())
            .await
            .unwrap()
            .is_none()
    );
    db.set_meta("after-rejected-rename", Some(json!(true)))
        .await
        .unwrap();
    db.close().await.unwrap();
}

#[tokio::test]
async fn deleting_a_source_preserves_identical_row_ids_in_other_projection_namespaces() {
    let input = fixture();
    let case = &input["cases"][4];
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    seed(&db, &case["seed"]).await;
    let mut definition = case["seed"]["definition"].clone();
    definition["projectionId"] = json!("fixture/other");
    db.register_projection_definition(serde_json::from_value(definition).unwrap())
        .await
        .unwrap();
    db.replace_projection_source(serde_json::from_value(json!({"projectionId":"fixture/other","sourcePath":"Links.md","sourceHash":"hash-1","rows":[{"id":"main","kind":"item","data":{"id":"main","label":"preserved"}}]})).unwrap()).await.unwrap();
    apply(&db, &case["steps"][0]).await.unwrap();
    let result=db.query_projection("fixture/other",serde_json::from_value(json!({"select":["id","label"],"where":{"op":"compare","field":"label","comparison":"eq","value":"preserved"}})).unwrap(),None).await.unwrap();
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0]["id"], "main");
    db.close().await.unwrap();
}
