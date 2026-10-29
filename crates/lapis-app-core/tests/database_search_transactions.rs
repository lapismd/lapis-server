use lapis_app_core::database::Database;
use lapis_app_protocol::{ErrorCode, database::search::records::SearchDocumentRecord};
use serde_json::json;
use std::{future::Future, task::Poll};
use uuid::Uuid;
mod search_support;
use search_support::{assert_json, fixture, seed};

#[tokio::test]
async fn search_publication_collision_rolls_back_document_chunks_revision_and_notification() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path(), Uuid::new_v4()).await.unwrap();
    seed(&db).await;
    let before = db.revision().await.unwrap();
    let mut changes = db.subscribe_to_changes();
    let mut bad = fixture()["records"][1]["stored"].clone();
    bad["checksum"] = json!("must-not-publish");
    bad["chunks"] = json!([
        {"id":"B.md#partial","text":"partial","startOffset":0,"endOffset":7},
        {"id":"A.md#ready","text":"collides","startOffset":7,"endOffset":15}
    ]);
    assert_eq!(
        db.upsert_prepared_search_document(serde_json::from_value(bad).unwrap())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Io
    );
    assert_eq!(db.revision().await.unwrap(), before);
    assert!(changes.try_recv().is_err());
    assert_json(
        serde_json::to_value(db.get_search_document("B.md").await.unwrap().unwrap()).unwrap(),
        fixture()["records"][1]["stored"].clone(),
    );
    assert_json(
        serde_json::to_value(db.get_search_index_stats().await.unwrap()).unwrap(),
        fixture()["stats"].clone(),
    );
    db.delete_search_document("B.md").await.unwrap();
    assert_eq!(changes.try_recv().unwrap().revision, before + 1);
    db.close().await.unwrap();
}

#[tokio::test]
async fn accepted_search_write_drains_after_waiter_detaches_and_closed_admission_cannot_publish() {
    let dir = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(dir.path(), id).await.unwrap();
    let record: SearchDocumentRecord =
        serde_json::from_value(fixture()["records"][0]["stored"].clone()).unwrap();
    let mut pending = Box::pin(db.upsert_prepared_search_document(record.clone()));
    std::future::poll_fn(|cx| {
        assert!(pending.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(pending);
    db.close().await.unwrap();
    assert_eq!(
        db.upsert_prepared_search_document(record)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Closed
    );
    let db = Database::open(dir.path(), id).await.unwrap();
    assert!(db.get_search_document("A.md").await.unwrap().is_some());
    assert_eq!(db.revision().await.unwrap(), 1);
    db.close().await.unwrap();
}

#[tokio::test]
async fn nonfinite_search_numbers_reject_before_writing_any_domain() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path(), Uuid::new_v4()).await.unwrap();
    let base: SearchDocumentRecord =
        serde_json::from_value(fixture()["records"][0]["stored"].clone()).unwrap();
    let mut bad = base.clone();
    bad.chunks.as_mut().unwrap()[0]
        .embedding
        .as_mut()
        .unwrap()
        .vector = Some(vec![f64::NAN]);
    assert_eq!(
        db.upsert_prepared_search_document(bad)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    let mut bad = base;
    bad.chunks.as_mut().unwrap()[0].start_offset = f64::INFINITY;
    assert_eq!(
        db.upsert_prepared_search_document(bad)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    let mut bad: SearchDocumentRecord =
        serde_json::from_value(fixture()["records"][1]["stored"].clone()).unwrap();
    bad.source_metadata.as_mut().unwrap().source_mtime = Some(f64::NEG_INFINITY);
    assert_eq!(
        db.upsert_prepared_search_document(bad)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(db.revision().await.unwrap(), 0);
    assert!(db.list_search_documents().await.unwrap().is_empty());
    db.close().await.unwrap();
}
