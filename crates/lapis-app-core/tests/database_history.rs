use lapis_app_core::database::Database;
use lapis_app_protocol::database::history::{HistoryEvent, StoreHistory};
use serde_json::json;
use uuid::Uuid;

fn input(path: &str, event: &str, content: &str, hash: Option<&str>) -> StoreHistory {
    let mut value = json!({"path":path,"eventType":event,"createdAt":10,"content":content});
    if let Some(hash) = hash {
        value["contentHash"] = json!(hash);
    }
    serde_json::from_value(value).unwrap()
}

#[tokio::test]
async fn native_history_retains_lineage_deduplication_and_rename_delete_restore_formats() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    assert!(db.get_file_history("old.md").await.unwrap().is_none());
    let first = db
        .store_file_history_revision(input("old.md", "baseline", "日本語 🪨", None))
        .await
        .unwrap();
    assert!(first.stored && !first.deduplicated);
    assert_eq!(first.revision.as_ref().unwrap().content_hash, "86fa74a6");
    let duplicate = db
        .store_file_history_revision(input("old.md", "modify", "日本語 🪨", Some("86fa74a6")))
        .await
        .unwrap();
    assert!(!duplicate.stored && duplicate.deduplicated);
    assert_eq!(db.revision().await.unwrap(), 1);
    let mut rename = input("new.md", "rename", "日本語 🪨", Some("86fa74a6"));
    rename.previous_path = Some("old.md".into());
    let renamed = db.store_file_history_revision(rename).await.unwrap();
    assert_eq!(renamed.file_id, first.file_id);
    assert_eq!(renamed.revision.unwrap().captured_path, "old.md");
    assert!(db.get_file_history("old.md").await.unwrap().is_none());
    let mut deletion = input("new.md", "delete", "", None);
    deletion.content = None;
    db.store_file_history_revision(deletion).await.unwrap();
    assert!(
        db.get_file_history("new.md")
            .await
            .unwrap()
            .unwrap()
            .file
            .deleted
    );
    db.store_file_history_revision(input(
        "new.md",
        "restore",
        "restored",
        Some("restored-hash"),
    ))
    .await
    .unwrap();
    let history = db.get_file_history("new.md").await.unwrap().unwrap();
    assert!(!history.file.deleted);
    assert_eq!(history.file.file_id, first.file_id);
    assert_eq!(history.revisions.len(), 4);
    assert_eq!(history.revisions[0].content, "日本語 🪨");
    assert_eq!(history.revisions[2].content, "日本語 🪨");
    assert!(matches!(
        history.revisions[3].event_type,
        HistoryEvent::Restore
    ));
    db.close().await.unwrap();
}

#[tokio::test]
async fn rejected_history_publication_rolls_back_lineage_revisions_and_change_events() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let engine =
        turso::Builder::new_local(profile.path().join(format!("{id}.db")).to_str().unwrap())
            .build()
            .await
            .unwrap();
    let sql = engine.connect().unwrap();
    sql.execute_batch("CREATE TABLE history_revisions(file_id TEXT NOT NULL,ordinal INTEGER NOT NULL,data_json TEXT NOT NULL CHECK(json_extract(data_json,'$.content')!='reject'),PRIMARY KEY(file_id,ordinal));").await.unwrap();
    drop(sql);
    drop(engine);
    let db = Database::open(profile.path(), id).await.unwrap();
    let mut events = db.subscribe_to_changes();
    let first = db
        .store_file_history_revision(input("old.md", "create", "retained", Some("one")))
        .await
        .unwrap();
    let event = events.recv().await.unwrap();
    assert_eq!(
        serde_json::to_value(event.domains).unwrap(),
        json!(["history"])
    );
    assert_eq!(event.paths, ["old.md"]);
    let mut failed = input("new.md", "rename", "reject", Some("two"));
    failed.previous_path = Some("old.md".into());
    assert_eq!(
        db.store_file_history_revision(failed)
            .await
            .unwrap_err()
            .code(),
        lapis_app_protocol::ErrorCode::Io
    );
    assert!(matches!(
        events.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    assert_eq!(db.revision().await.unwrap(), 1);
    assert!(db.get_file_history("new.md").await.unwrap().is_none());
    let retained = db.get_file_history("old.md").await.unwrap().unwrap();
    assert_eq!(retained.file.file_id, first.file_id);
    assert_eq!(retained.revisions.len(), 1);
    assert_eq!(retained.revisions[0].content, "retained");
    db.close().await.unwrap();
}

#[tokio::test]
async fn invalid_history_rows_and_numbers_reject_without_discarding_database_only_records() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let engine =
        turso::Builder::new_local(profile.path().join(format!("{id}.db")).to_str().unwrap())
            .build()
            .await
            .unwrap();
    let sql = engine.connect().unwrap();
    sql.execute_batch("CREATE TABLE history_files(file_id TEXT PRIMARY KEY,data_json TEXT NOT NULL); CREATE TABLE history_file_paths(path TEXT PRIMARY KEY,file_id TEXT NOT NULL); INSERT INTO history_files(file_id,data_json) VALUES('existing','{broken'); INSERT INTO history_file_paths(path,file_id) VALUES('note.md','existing');").await.unwrap();
    drop(sql);
    drop(engine);
    let db = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(
        db.get_file_history("note.md").await.unwrap_err().code(),
        lapis_app_protocol::ErrorCode::InvalidRequest
    );
    assert_eq!(
        db.store_file_history_revision(input("note.md", "modify", "new", Some("new")))
            .await
            .unwrap_err()
            .code(),
        lapis_app_protocol::ErrorCode::InvalidRequest
    );
    let mut bad = input("new.md", "create", "new", Some("new"));
    bad.created_at = f64::NAN;
    assert_eq!(
        db.store_file_history_revision(bad)
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
        .query(
            "SELECT data_json FROM history_files WHERE file_id='existing'",
            (),
        )
        .await
        .unwrap();
    assert_eq!(
        rows.next()
            .await
            .unwrap()
            .unwrap()
            .get::<String>(0)
            .unwrap(),
        "{broken"
    );
}

#[tokio::test]
async fn latest_history_replacement_and_retention_preserve_exact_pins_after_reopen() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    let first = db
        .store_file_history_revision(input("note.md", "create", "one", Some("one")))
        .await
        .unwrap()
        .revision
        .unwrap();
    let mut replacement = input("note.md", "modify", "two", Some("two"));
    replacement.replace_latest = true;
    let replaced = db
        .store_file_history_revision(replacement)
        .await
        .unwrap()
        .revision
        .unwrap();
    assert_eq!(replaced.revision_id, first.revision_id);
    assert_eq!(
        db.get_file_history("note.md")
            .await
            .unwrap()
            .unwrap()
            .revisions
            .len(),
        1
    );
    let mut latest = input("note.md", "modify", "three", Some("three"));
    latest.max_revisions = Some(0.0);
    let latest = db
        .store_file_history_revision(latest)
        .await
        .unwrap()
        .revision
        .unwrap();
    assert_ne!(latest.revision_id, first.revision_id);
    db.close().await.unwrap();
    let db = Database::open(profile.path(), id).await.unwrap();
    let history = db.get_file_history("note.md").await.unwrap().unwrap();
    assert_eq!(history.revisions.len(), 1);
    assert_eq!(history.revisions[0].revision_id, latest.revision_id);
    assert_eq!(history.revisions[0].content, "three");
    db.close().await.unwrap();
}
