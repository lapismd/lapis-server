use lapis_app_core::database::Database;
use lapis_app_protocol::{ErrorCode, database::search::records::SearchDocumentRecord};
use serde_json::{Value, json};
use std::{future::Future, task::Poll};
use uuid::Uuid;

fn document(index: usize) -> SearchDocumentRecord {
    serde_json::from_value(json!({"path":format!("{index:03}.md"),"name":"Note","extension":"md","checksum":"source","content":"A new note.","tags":["stale"],"tagParts":[],"tagHierarchy":[],"chunks":[{"id":format!("{index}#old"),"text":"Old text","startOffset":0,"endOffset":8}],"sourceMetadata":{"rawTags":["#new","topic/child"],"frontmatter":{"status":"ready"},"headings":[],"sections":[],"chunking":{"targetChars":1000,"breakpointWindowChars":0,"breakpointDecay":1}}})).unwrap()
}

async fn configure(db: &Database) {
    db.configure_search_embedding_provider(Some(
        serde_json::from_value(json!({"kind":"token-hash","dimensions":12})).unwrap(),
    ))
    .await
    .unwrap();
}

#[tokio::test]
async fn admitted_batch_barriers_do_not_write_and_empty_rebuild_publishes_once() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let mut events = db.subscribe_to_changes();
    for _ in 0..2 {
        db.begin_search_indexing_batch().await.unwrap();
    }
    for _ in 0..3 {
        db.end_search_indexing_batch().await.unwrap();
    }
    assert_eq!(db.revision().await.unwrap(), 0);
    assert!(events.try_recv().is_err());
    db.rebuild_search_index().await.unwrap();
    let event = events.try_recv().unwrap();
    assert_eq!(event.revision, 1);
    assert_eq!(
        serde_json::to_value(event.domains).unwrap(),
        json!(["search"])
    );
    assert!(event.paths.is_empty());
    assert_eq!(db.get_search_index_stats().await.unwrap().document_count, 0);
    assert!(events.try_recv().is_err());
    db.close().await.unwrap();
    assert_eq!(
        db.begin_search_indexing_batch().await.unwrap_err().code(),
        ErrorCode::Closed
    );
    assert_eq!(
        db.end_search_indexing_batch().await.unwrap_err().code(),
        ErrorCode::Closed
    );
    assert_eq!(
        db.rebuild_search_index().await.unwrap_err().code(),
        ErrorCode::Closed
    );
}

#[tokio::test]
async fn detached_rebuild_reprepares_all_pages_before_close_and_survives_reopen() {
    let profile = tempfile::tempdir().unwrap();
    let vault = Uuid::new_v4();
    let db = Database::open(profile.path(), vault).await.unwrap();
    for index in 0..103 {
        db.upsert_prepared_search_document(document(index))
            .await
            .unwrap();
    }
    configure(&db).await;
    let before = db.revision().await.unwrap();
    let mut events = db.subscribe_to_changes();
    let mut pending = Box::pin(db.rebuild_search_index());
    std::future::poll_fn(|context| {
        assert!(pending.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(pending);
    db.close().await.unwrap();
    let first = events.try_recv().unwrap();
    let last = events.try_recv().unwrap();
    assert_eq!((first.revision, last.revision), (before + 1, before + 2));
    assert_eq!(first.paths.len(), 100);
    assert_eq!(last.paths, ["100.md", "101.md", "102.md"]);
    assert!(events.try_recv().is_err());
    let db = Database::open(profile.path(), vault).await.unwrap();
    assert_eq!(db.revision().await.unwrap(), before + 2);
    let stats = db.get_search_index_stats().await.unwrap();
    assert_eq!(
        (
            stats.document_count,
            stats.chunk_count,
            stats.ready_chunk_count
        ),
        (103, 103, 103)
    );
    for row in db.list_search_documents().await.unwrap() {
        assert_eq!(row.tags, ["new", "topic/child"]);
        assert_eq!(row.tag_hierarchy, ["new", "topic", "topic/child"]);
        assert_eq!(row.metadata_text.as_deref(), Some("{\"status\":\"ready\"}"));
        let chunk = row.chunks.unwrap().pop().unwrap();
        assert_eq!(chunk.text, "A new note.");
        let vector = chunk.embedding.unwrap().vector.unwrap();
        assert_eq!(vector.len(), 12);
        assert!((vector.iter().map(|n| n * n).sum::<f64>() - 1.).abs() < 1e-12);
    }
    assert_eq!(
        db.search_document_paths(
            "new",
            serde_json::from_value(json!({"mode":"lexical","limit":200})).unwrap()
        )
        .await
        .unwrap()
        .len(),
        103
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn failed_rebuild_page_preserves_earlier_commits_and_keeps_owner_usable() {
    let profile = tempfile::tempdir().unwrap();
    let vault = Uuid::new_v4();
    let db = Database::open(profile.path(), vault).await.unwrap();
    for index in 0..102 {
        db.upsert_prepared_search_document(document(index))
            .await
            .unwrap();
    }
    configure(&db).await;
    let store =
        turso::Builder::new_local(profile.path().join(format!("{vault}.db")).to_str().unwrap())
            .build()
            .await
            .unwrap();
    let connection = store.connect().unwrap();
    connection
        .execute(
            "UPDATE search_docs SET data_json='broken' WHERE path='101.md'",
            (),
        )
        .await
        .unwrap();
    let before = db.revision().await.unwrap();
    let mut events = db.subscribe_to_changes();
    assert_eq!(
        db.rebuild_search_index().await.unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(db.revision().await.unwrap(), before + 1);
    let event = events.try_recv().unwrap();
    assert_eq!(event.revision, before + 1);
    assert_eq!(event.paths.len(), 100);
    assert_eq!(event.paths.last().unwrap(), "099.md");
    assert!(events.try_recv().is_err());
    let stats = db.get_search_index_stats().await.unwrap();
    assert_eq!(
        (stats.ready_chunk_count, stats.pending_chunk_count),
        (100, 2)
    );
    db.set_meta("still-alive", Some(json!(true))).await.unwrap();
    drop(connection);
    drop(store);
    db.close().await.unwrap();
}

#[tokio::test]
async fn failed_publication_rolls_back_every_document_in_its_page() {
    let profile = tempfile::tempdir().unwrap();
    let vault = Uuid::new_v4();
    let db = Database::open(profile.path(), vault).await.unwrap();
    for index in 0..2 {
        db.upsert_prepared_search_document(document(index))
            .await
            .unwrap();
    }
    configure(&db).await;
    let before = serde_json::to_value(db.list_search_documents().await.unwrap()).unwrap();
    let store =
        turso::Builder::new_local(profile.path().join(format!("{vault}.db")).to_str().unwrap())
            .build()
            .await
            .unwrap();
    let connection = store.connect().unwrap();
    connection
        .execute(
            "UPDATE app_revision SET revision=9007199254740991 WHERE id=1",
            (),
        )
        .await
        .unwrap();
    let mut events = db.subscribe_to_changes();
    assert_eq!(
        db.rebuild_search_index().await.unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    let after = serde_json::to_value(db.list_search_documents().await.unwrap()).unwrap();
    assert_eq!(before, after);
    assert!(events.try_recv().is_err());
    drop(connection);
    drop(store);
    db.close().await.unwrap();
}

#[tokio::test]
async fn rebuild_keeps_missing_model_failure_explicit_without_hash_fallback() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    db.upsert_prepared_search_document(document(0))
        .await
        .unwrap();
    db.configure_search_embedding_provider(Some(
        serde_json::from_value(
            json!({"kind":"transformers-js","modelId":"Xenova/all-MiniLM-L6-v2","dimensions":384}),
        )
        .unwrap(),
    ))
    .await
    .unwrap();
    db.rebuild_search_index().await.unwrap();
    let row: Value = serde_json::to_value(db.get_search_document("000.md").await.unwrap()).unwrap();
    let embedding = &row["chunks"][0]["embedding"];
    assert_eq!(embedding["status"], "error");
    assert!(embedding.get("vector").is_none());
    assert_eq!(
        db.get_search_index_stats().await.unwrap().error_chunk_count,
        1
    );
    db.close().await.unwrap();
}
