use lapis_app_core::database::{Database, MetaCheck, MetaMutation, MetaTransaction};
use lapis_app_protocol::{
    ErrorCode,
    database::state::{NotebookState, NotificationRecord},
};
use serde_json::json;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use uuid::Uuid;

#[tokio::test]
async fn rejected_metadata_checks_do_not_publish_and_subscriptions_close_after_drain() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let mut events = db.subscribe_to_changes();
    assert!(
        !db.transact_meta(MetaTransaction {
            checks: vec![MetaCheck {
                key: "absent".into(),
                exists: Some(true),
                value: None
            }],
            mutations: vec![MetaMutation {
                key: "uncommitted".into(),
                value: Some(json!(true)),
                delete: false
            }]
        })
        .await
        .unwrap()
    );
    assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
    assert_eq!(db.revision().await.unwrap(), 0);
    db.close().await.unwrap();
    assert!(matches!(events.recv().await, Err(RecvError::Closed)));
}

#[tokio::test]
async fn slow_subscribers_get_explicit_lag_without_blocking_commits() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let mut events = db.subscribe_to_changes();
    for n in 0..140 {
        db.set_meta("counter", Some(json!(n))).await.unwrap();
    }
    assert!(matches!(events.recv().await, Err(RecvError::Lagged(12))));
    assert_eq!(db.revision().await.unwrap(), 140);
    assert_eq!(db.get_meta("counter").await.unwrap(), Some(json!(139)));
    db.close().await.unwrap();
}

#[tokio::test]
async fn notebook_changes_carry_the_source_path_and_large_detail_resets_the_domain() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let mut events = db.subscribe_to_changes();
    let state: NotebookState =
        serde_json::from_value(json!({"sourcePath":"note.md","mtime":1,"updatedAt":2,"cells":{}}))
            .unwrap();
    db.set_notebook_state("note.md", state.clone())
        .await
        .unwrap();
    let event = events.recv().await.unwrap();
    assert_eq!(event.paths, ["note.md"]);
    assert_eq!(
        serde_json::to_value(event.domains).unwrap(),
        json!(["notebook"])
    );
    db.set_notebook_state(&"x".repeat(70_000), state)
        .await
        .unwrap();
    let event = events.recv().await.unwrap();
    assert!(event.paths.is_empty());
    assert_eq!(event.reset, Some(true));
    db.close().await.unwrap();
}

#[tokio::test]
async fn notification_batch_failure_rolls_back_every_record_and_publishes_nothing() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let engine =
        turso::Builder::new_local(profile.path().join(format!("{id}.db")).to_str().unwrap())
            .build()
            .await
            .unwrap();
    let sql = engine.connect().unwrap();
    sql.execute_batch("CREATE TABLE notifications(id TEXT PRIMARY KEY,data_json TEXT NOT NULL CHECK(id!='blocked' OR json_extract(data_json,'$.cleared')=0));").await.unwrap();
    for name in ["first", "blocked"] {
        let record = json!({"id":name,"message":"retained","severity":"info","createdAt":1,"updatedAt":1,"read":false,"cleared":false});
        sql.execute(
            "INSERT INTO notifications(id,data_json) VALUES(?,?)",
            turso::params![name, record.to_string()],
        )
        .await
        .unwrap();
    }
    drop(sql);
    drop(engine);
    let db = Database::open(profile.path(), id).await.unwrap();
    let mut events = db.subscribe_to_changes();
    assert_eq!(
        db.clear_all_notifications().await.unwrap_err().code(),
        ErrorCode::Io
    );
    assert_eq!(db.list_notifications().await.unwrap().len(), 2);
    assert_eq!(db.revision().await.unwrap(), 0);
    assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
    db.mark_notification_read("first").await.unwrap();
    assert_eq!(events.recv().await.unwrap().revision, 1);
    db.close().await.unwrap();
}

#[tokio::test]
async fn invalid_numbers_and_corrupt_state_reject_without_clearing_stored_values() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let mut notice:NotificationRecord=serde_json::from_value(json!({"id":"bad","message":"bad","severity":"info","createdAt":1,"updatedAt":1,"read":false,"cleared":false})).unwrap();
    notice.created_at = f64::NAN;
    assert_eq!(
        db.upsert_notification(notice).await.unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    let mut state: NotebookState =
        serde_json::from_value(json!({"sourcePath":"bad","mtime":1,"updatedAt":2,"cells":{}}))
            .unwrap();
    state.mtime = f64::INFINITY;
    assert_eq!(
        db.set_notebook_state("bad", state)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(db.revision().await.unwrap(), 0);
    db.set_meta("notebook.state:bad", Some(json!(true)))
        .await
        .unwrap();
    assert_eq!(
        db.get_notebook_state("bad").await.unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        db.get_meta("notebook.state:bad").await.unwrap(),
        Some(json!(true))
    );
    db.close().await.unwrap();
}
