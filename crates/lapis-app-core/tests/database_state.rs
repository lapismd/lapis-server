use lapis_app_core::database::Database;
use lapis_app_protocol::database::state::{MetadataSnapshot, NotebookState, NotificationRecord};
use serde_json::json;
use uuid::Uuid;

fn notification(id: &str, created_at: i64) -> NotificationRecord {
    serde_json::from_value(json!({"id":id,"message":"A notification","severity":"info","createdAt":created_at,"updatedAt":created_at,"read":false,"cleared":false})).unwrap()
}

#[tokio::test]
async fn native_state_retains_notebook_outputs_and_deprecated_snapshot_without_using_it_for_other_data()
 {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    assert!(db.get_notebook_state("note.md").await.unwrap().is_none());
    assert!(db.load_metadata_snapshot().await.unwrap().is_none());
    let state: NotebookState = serde_json::from_value(json!({"sourcePath":"note.md","mtime":12.5,"updatedAt":20,"cells":{"one":{"state":"done","outputs":[{"type":"text","value":"日本語 🪨"}],"inputValues":{"enabled":true},"setupPhases":[{"key":"setup","label":"Setup","status":"done","durationMs":0.5}]}}})).unwrap();
    db.set_notebook_state("note.md", state.clone())
        .await
        .unwrap();
    let snapshot: MetadataSnapshot = serde_json::from_value(json!({"fileCache":{},"metadataCache":{"snapshot-only":{"value":true}},"resolvedLinks":{},"unresolvedLinks":{}})).unwrap();
    db.save_metadata_snapshot(snapshot).await.unwrap();
    assert_eq!(db.get_meta("snapshot-only").await.unwrap(), None);
    assert_eq!(
        serde_json::to_value(db.get_notebook_state("note.md").await.unwrap().unwrap()).unwrap(),
        serde_json::to_value(state).unwrap()
    );
    db.close().await.unwrap();
    let db = Database::open(profile.path(), id).await.unwrap();
    assert!(db.load_metadata_snapshot().await.unwrap().is_some());
    assert!(db.get_notebook_state("note.md").await.unwrap().is_some());
    db.delete_notebook_state("note.md").await.unwrap();
    assert!(db.get_notebook_state("note.md").await.unwrap().is_none());
    db.close().await.unwrap();
}

#[tokio::test]
async fn native_notification_updates_preserve_order_read_clear_and_one_bulk_revision() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    assert!(db.list_notifications().await.unwrap().is_empty());
    db.mark_notification_read("missing").await.unwrap();
    db.clear_notification("missing").await.unwrap();
    assert_eq!(db.revision().await.unwrap(), 0);
    db.clear_all_notifications().await.unwrap();
    assert_eq!(db.revision().await.unwrap(), 1);
    db.upsert_notification(notification("older", 10))
        .await
        .unwrap();
    db.upsert_notification(notification("newer", 20))
        .await
        .unwrap();
    let records = db.list_notifications().await.unwrap();
    assert_eq!(
        records.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
        ["newer", "older"]
    );
    db.mark_notification_read("newer").await.unwrap();
    assert!(db.list_notifications().await.unwrap()[0].read);
    db.clear_notification("older").await.unwrap();
    assert_eq!(db.list_notifications().await.unwrap().len(), 1);
    let revision = db.revision().await.unwrap();
    db.clear_all_notifications().await.unwrap();
    assert!(db.list_notifications().await.unwrap().is_empty());
    assert_eq!(db.revision().await.unwrap(), revision + 1);
    db.clear_all_notifications().await.unwrap();
    assert_eq!(db.revision().await.unwrap(), revision + 2);
    db.close().await.unwrap();
}

#[tokio::test]
async fn deprecated_snapshot_explicitly_reads_prior_app_state_and_new_value_takes_precedence() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let engine =
        turso::Builder::new_local(profile.path().join(format!("{id}.db")).to_str().unwrap())
            .build()
            .await
            .unwrap();
    let sql = engine.connect().unwrap();
    sql.execute_batch(
        "CREATE TABLE app_state(id INTEGER PRIMARY KEY CHECK(id=1),state_json TEXT NOT NULL);",
    )
    .await
    .unwrap();
    let old = json!({"fileCache":{},"metadataCache":{"old":true},"resolvedLinks":{},"unresolvedLinks":{}});
    let original = json!({"metadataSnapshot":old,"otherDomain":"retained"}).to_string();
    sql.execute(
        "INSERT INTO app_state(id,state_json) VALUES(1,?)",
        [original.as_str()],
    )
    .await
    .unwrap();
    drop(sql);
    drop(engine);
    let db = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(
        serde_json::to_value(db.load_metadata_snapshot().await.unwrap().unwrap()).unwrap(),
        old
    );
    assert_eq!(db.get_meta("old").await.unwrap(), None);
    let new = json!({"fileCache":{},"metadataCache":{"new":true},"resolvedLinks":{},"unresolvedLinks":{}});
    db.save_metadata_snapshot(serde_json::from_value(new.clone()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(db.load_metadata_snapshot().await.unwrap().unwrap()).unwrap(),
        new
    );
    db.close().await.unwrap();
    let engine =
        turso::Builder::new_local(profile.path().join(format!("{id}.db")).to_str().unwrap())
            .build()
            .await
            .unwrap();
    let sql = engine.connect().unwrap();
    let mut rows = sql
        .query("SELECT state_json FROM app_state WHERE id=1", ())
        .await
        .unwrap();
    assert_eq!(
        rows.next()
            .await
            .unwrap()
            .unwrap()
            .get::<String>(0)
            .unwrap(),
        original
    );
}

#[tokio::test]
async fn corrupt_notification_identity_is_rejected_without_republishing_under_another_id() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let engine =
        turso::Builder::new_local(profile.path().join(format!("{id}.db")).to_str().unwrap())
            .build()
            .await
            .unwrap();
    let sql = engine.connect().unwrap();
    sql.execute_batch("CREATE TABLE notifications(id TEXT PRIMARY KEY,data_json TEXT NOT NULL);")
        .await
        .unwrap();
    let value = serde_json::to_string(&notification("other-id", 1)).unwrap();
    sql.execute(
        "INSERT INTO notifications(id,data_json) VALUES('original',?)",
        [value.as_str()],
    )
    .await
    .unwrap();
    drop(sql);
    drop(engine);
    let db = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(
        db.list_notifications().await.unwrap_err().code(),
        lapis_app_protocol::ErrorCode::InvalidRequest
    );
    assert_eq!(
        db.mark_notification_read("original")
            .await
            .unwrap_err()
            .code(),
        lapis_app_protocol::ErrorCode::InvalidRequest
    );
    assert_eq!(db.revision().await.unwrap(), 0);
    db.close().await.unwrap();
    let engine =
        turso::Builder::new_local(profile.path().join(format!("{id}.db")).to_str().unwrap())
            .build()
            .await
            .unwrap();
    let sql = engine.connect().unwrap();
    let mut rows = sql
        .query("SELECT id,data_json FROM notifications", ())
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert_eq!(row.get::<String>(0).unwrap(), "original");
    assert_eq!(row.get::<String>(1).unwrap(), value);
    assert!(rows.next().await.unwrap().is_none());
}

#[tokio::test]
async fn changes_publish_committed_domain_and_path_after_observers_detach() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let mut events = db.subscribe_to_changes();
    db.set_meta("setting", Some(json!(true))).await.unwrap();
    let event = events.recv().await.unwrap();
    assert_eq!(
        serde_json::to_value(event).unwrap()["domains"],
        json!(["meta"])
    );
    db.upsert_notification(notification("first", 10))
        .await
        .unwrap();
    let event = events.recv().await.unwrap();
    assert_eq!(event.revision, db.revision().await.unwrap());
    assert_eq!(
        serde_json::to_value(event).unwrap()["domains"],
        json!(["notification"])
    );
    drop(events);
    db.upsert_notification(notification("no-observer", 20))
        .await
        .unwrap();
    assert_eq!(db.list_notifications().await.unwrap().len(), 2);
    db.close().await.unwrap();
}
