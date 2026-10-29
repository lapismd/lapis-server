use super::support::*;
use crate::services::indexing::owner::Clock;
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn explicit_native_refresh_rebuild_reprepares_unchanged_stored_sources() {
    use lapis_app_protocol::indexing::IndexingCommand;
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "Note.md", "# Native rebuild\n\nunchanged content\n");
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    select(&vault, true).await;
    let db = database(&vault).await;
    until(async || {
        db.get_search_document("Note.md")
            .await
            .unwrap()
            .is_some_and(|row| row.chunks.is_some_and(|chunks| !chunks.is_empty()))
    })
    .await;
    let mut row = db.get_search_document("Note.md").await.unwrap().unwrap();
    // Lose only derived chunks while retaining the source and manifest identity.
    row.chunks = Some(vec![]);
    db.upsert_prepared_search_document(row).await.unwrap();
    let client = vault.attach().unwrap();
    client
        .control_indexing(IndexingCommand::Refresh { rebuild: false })
        .await
        .unwrap();
    assert!(
        db.get_search_document("Note.md")
            .await
            .unwrap()
            .unwrap()
            .chunks
            .unwrap()
            .is_empty()
    );
    client
        .control_indexing(IndexingCommand::Refresh { rebuild: true })
        .await
        .unwrap();
    let rebuilt = db.get_search_document("Note.md").await.unwrap().unwrap();
    assert!(
        !rebuilt.chunks.unwrap().is_empty(),
        "explicit native rebuild must repair derived chunks even when source manifests match"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("Note.md")).unwrap(),
        "# Native rebuild\n\nunchanged content\n"
    );
    close(runtime).await;
}

#[tokio::test]
async fn periodic_reconciliation_repairs_a_missing_search_cache_without_a_watcher_hint() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "Note.md", "# Periodic repair\n");
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault
        .start_indexing(
            false,
            false,
            Clock {
                interval: Duration::from_millis(25),
                ..Clock::default()
            },
        )
        .await
        .unwrap();
    select(&vault, true).await;
    let db = database(&vault).await;
    until(async || db.get_search_document("Note.md").await.unwrap().is_some()).await;
    db.delete_search_document("Note.md").await.unwrap();
    until(async || db.get_search_document("Note.md").await.unwrap().is_some()).await;
    assert_eq!(vault.status().unwrap().clients, 0);
    close(runtime).await;
}

#[tokio::test]
async fn selected_source_reuses_the_explicitly_configured_database_embedding_engine() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    write(&root, "Note.md", "# Pear\n\nGreen fruit in the garden.\n");
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let db = database(&vault).await;
    // Deterministic source plumbing only; this does not substitute for ONNX qualification.
    db.configure_search_embedding_provider(Some(
        serde_json::from_value(json!({
            "kind":"token-hash","modelId":"lapis/token-hash-v0","dimensions":48
        }))
        .unwrap(),
    ))
    .await
    .unwrap();
    select(&vault, true).await;
    until(async || db.get_search_document("Note.md").await.unwrap().is_some()).await;
    let row = db.get_search_document("Note.md").await.unwrap().unwrap();
    let chunks = row.chunks.unwrap();
    assert!(!chunks.is_empty());
    for chunk in chunks {
        let embedding = chunk.embedding.unwrap();
        assert_eq!(embedding.model_id, "lapis/token-hash-v0");
        assert_eq!(
            serde_json::to_value(embedding.status).unwrap(),
            json!("ready")
        );
        assert_eq!(embedding.vector.unwrap().len(), 48);
    }
    let revision = db.revision().await.unwrap();
    rescan(&vault).await;
    assert_eq!(db.revision().await.unwrap(), revision);
    close(runtime).await;
}
