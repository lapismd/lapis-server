use lapis_app_core::{database::Database, embedding::host::NativeEmbeddingHost};
use lapis_app_protocol::database::search::embedding::SearchEmbeddingProviderConfig;
use serde_json::{Value, json};
use uuid::Uuid;

#[tokio::test]
async fn managed_cache_is_lazy_and_offline_missing_assets_do_not_fall_back() {
    let cache = tempfile::tempdir().unwrap();
    let host = NativeEmbeddingHost::managed(cache.path()).unwrap();
    assert_eq!(std::fs::read_dir(cache.path()).unwrap().count(), 0);
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open_with_embeddings(profile.path(), Uuid::new_v4(), host)
        .await
        .unwrap();
    let config: SearchEmbeddingProviderConfig = serde_json::from_value(json!({
        "kind":"transformers-js", "modelId":"Xenova/all-MiniLM-L6-v2",
        "dimensions":384, "allowRemoteModels":false
    }))
    .unwrap();
    db.configure_search_embedding_provider(Some(config))
        .await
        .unwrap();
    assert_eq!(std::fs::read_dir(cache.path()).unwrap().count(), 0);
    db.upsert_search_document(serde_json::from_value(json!({
        "path":"Note.md", "name":"Note", "extension":"md", "checksum":"one",
        "content":"A cat at the window.", "tags":[], "tagParts":[], "tagHierarchy":[],
        "chunks":[{"id":"Note.md#0", "text":"A cat at the window.", "startOffset":0, "endOffset":20}]
    })).unwrap()).await.unwrap();
    let doc: Value =
        serde_json::to_value(db.get_search_document("Note.md").await.unwrap()).unwrap();
    assert_eq!(doc["chunks"][0]["embedding"]["status"], "error");
    assert!(doc["chunks"][0]["embedding"].get("vector").is_none());
    let status = db.get_search_embedding_runtime_status().unwrap().unwrap();
    assert_eq!(serde_json::to_value(&status).unwrap()["phase"], "error");
    assert!(
        !status
            .message
            .unwrap()
            .contains(cache.path().to_str().unwrap())
    );
    assert!(
        std::fs::read_dir(cache.path())
            .unwrap()
            .all(|entry| entry.unwrap().file_name() == "install.lock")
    );
    db.close().await.unwrap();
}
