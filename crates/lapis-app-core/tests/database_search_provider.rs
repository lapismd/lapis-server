use lapis_app_core::database::Database;
use lapis_app_protocol::{ErrorCode, database::search::embedding::SearchEmbeddingProviderConfig};
use serde_json::{Value, json};
use uuid::Uuid;

fn config(dimensions: usize) -> SearchEmbeddingProviderConfig {
    serde_json::from_value(json!({"kind":"transformers-js","modelId":"Xenova/all-MiniLM-L6-v2","dimensions":dimensions})).unwrap()
}
fn document() -> lapis_app_protocol::database::search::records::SearchDocumentRecord {
    serde_json::from_value(json!({"path":"Note.md","name":"Note","extension":"md","checksum":"one","content":"A cat on a window.","tags":[],"tagParts":[],"tagHierarchy":[],"chunks":[{"id":"Note.md#0","text":"A cat on a window.","startOffset":0,"endOffset":18}]})).unwrap()
}
#[tokio::test]
async fn configuration_is_lazy_persisted_and_idempotent_without_admitted_assets() {
    let directory = tempfile::tempdir().unwrap();
    let vault = Uuid::new_v4();
    let db = Database::open(directory.path(), vault).await.unwrap();
    assert!(db.get_search_embedding_provider().await.unwrap().is_none());
    assert!(db.get_search_embedding_runtime_status().unwrap().is_none());
    db.configure_search_embedding_provider(Some(config(384)))
        .await
        .unwrap();
    let revision = db.revision().await.unwrap();
    db.configure_search_embedding_provider(Some(config(384)))
        .await
        .unwrap();
    assert_eq!(db.revision().await.unwrap(), revision);
    let status = serde_json::to_value(db.get_search_embedding_runtime_status().unwrap()).unwrap();
    assert_eq!(status["phase"], "downloading");
    db.close().await.unwrap();
    assert_eq!(
        db.get_search_embedding_runtime_status().unwrap_err().code(),
        ErrorCode::Closed
    );
    let db = Database::open(directory.path(), vault).await.unwrap();
    assert_eq!(
        serde_json::to_value(db.get_search_embedding_provider().await.unwrap()).unwrap()["dimensions"],
        384
    );
    assert_eq!(
        serde_json::to_value(db.get_search_embedding_runtime_status().unwrap()).unwrap()["phase"],
        "downloading"
    );
    db.close().await.unwrap();
}
#[tokio::test]
async fn unavailable_model_publishes_error_without_a_replacement_vector() {
    let directory = tempfile::tempdir().unwrap();
    let db = Database::open(directory.path(), Uuid::new_v4())
        .await
        .unwrap();
    db.configure_search_embedding_provider(Some(config(384)))
        .await
        .unwrap();
    db.upsert_search_document(document()).await.unwrap();
    let doc = serde_json::to_value(db.get_search_document("Note.md").await.unwrap()).unwrap();
    let embedding = &doc["chunks"][0]["embedding"];
    assert_eq!(embedding["status"], "error");
    assert_eq!(embedding["modelId"], "Xenova/all-MiniLM-L6-v2");
    assert_eq!(embedding["dirty"], true);
    assert!(embedding.get("vector").is_none());
    assert!(
        !embedding["error"]
            .as_str()
            .unwrap()
            .contains(directory.path().to_str().unwrap())
    );
    assert_eq!(
        db.get_search_index_stats().await.unwrap().error_chunk_count,
        1
    );
    let status: Value =
        serde_json::to_value(db.get_search_embedding_runtime_status().unwrap()).unwrap();
    assert_eq!(status["phase"], "error");
    db.close().await.unwrap();
}

#[tokio::test]
async fn configuration_invalidates_all_pages_and_its_failure_retains_selected_runtime() {
    let directory = tempfile::tempdir().unwrap();
    let vault = Uuid::new_v4();
    let db = Database::open(directory.path(), vault).await.unwrap();
    let initial =
        serde_json::from_value(json!({"kind":"token-hash","modelId":"first","dimensions":12}))
            .unwrap();
    db.configure_search_embedding_provider(Some(initial))
        .await
        .unwrap();
    for index in 0..103 {
        let mut doc = document();
        doc.path = if index == 0 {
            String::new()
        } else {
            format!("Note{index:03}.md")
        };
        doc.chunks.as_mut().unwrap()[0].id = format!("{}#0", doc.path);
        db.upsert_search_document(doc).await.unwrap();
    }
    assert_eq!(
        db.get_search_index_stats().await.unwrap().ready_chunk_count,
        103
    );
    let next = config(384);
    let mut changes = db.subscribe_to_changes();
    db.configure_search_embedding_provider(Some(next.clone()))
        .await
        .unwrap();
    let event = changes.try_recv().unwrap();
    assert_eq!(
        serde_json::to_value(event.domains).unwrap(),
        json!(["search"])
    );
    assert!(changes.try_recv().is_err());
    for row in db.list_search_documents().await.unwrap() {
        let embedding = &row.chunks.unwrap()[0].embedding;
        let value = serde_json::to_value(embedding).unwrap();
        assert_eq!(value["status"], "pending");
        assert_eq!(value["dirty"], true);
        assert!(value.get("vector").is_none());
    }
    assert_eq!(
        db.get_search_index_stats()
            .await
            .unwrap()
            .pending_chunk_count,
        103
    );
    let store = turso::Builder::new_local(
        directory
            .path()
            .join(format!("{vault}.db"))
            .to_str()
            .unwrap(),
    )
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
    let before = serde_json::to_value(db.list_search_documents().await.unwrap()).unwrap();
    assert_eq!(
        db.configure_search_embedding_provider(Some(config(768)))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        serde_json::to_value(db.get_search_embedding_provider().await.unwrap()).unwrap(),
        serde_json::to_value(next).unwrap()
    );
    assert_eq!(
        serde_json::to_value(db.list_search_documents().await.unwrap()).unwrap(),
        before
    );
    assert!(changes.try_recv().is_err());
    drop(connection);
    drop(store);
    db.close().await.unwrap();
}

#[tokio::test]
async fn invalid_configuration_is_rejected_before_mutation() {
    let directory = tempfile::tempdir().unwrap();
    let db = Database::open(directory.path(), Uuid::new_v4())
        .await
        .unwrap();
    for mut invalid in [config(0), config(65537), config(384)] {
        if invalid.dimensions == Some(384) {
            invalid.model_id = None;
        }
        assert_eq!(
            db.configure_search_embedding_provider(Some(invalid))
                .await
                .unwrap_err()
                .code(),
            ErrorCode::InvalidRequest
        );
        assert!(db.get_search_embedding_provider().await.unwrap().is_none());
        assert_eq!(db.revision().await.unwrap(), 0);
    }
    db.close().await.unwrap();
}

#[tokio::test]
async fn explicit_token_hash_matches_the_retained_unicode_reference_without_model_fallback() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/search-semantic-model.json"
    ))
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let db = Database::open(directory.path(), Uuid::new_v4())
        .await
        .unwrap();
    db.configure_search_embedding_provider(Some(
        serde_json::from_value(fixture["tokenHash"]["config"].clone()).unwrap(),
    ))
    .await
    .unwrap();
    for (index, row) in fixture["queries"].as_array().unwrap().iter().enumerate() {
        let mut record = document();
        record.chunks.as_mut().unwrap()[0].text = row["text"].as_str().unwrap().into();
        db.upsert_search_document(record).await.unwrap();
        let row = db.get_search_document("Note.md").await.unwrap().unwrap();
        let embedding = row.chunks.unwrap().pop().unwrap().embedding.unwrap();
        let vector = embedding.vector.unwrap();
        for (actual, expected) in vector.iter().zip(
            fixture["tokenHash"]["vectors"][index]["vector"]
                .as_array()
                .unwrap(),
        ) {
            assert!((actual - expected.as_f64().unwrap()).abs() < 0.000000000001);
        }
        assert_eq!(
            embedding.fingerprint.as_deref(),
            fixture["tokenHash"]["vectors"][index]["fingerprint"].as_str()
        );
        assert_eq!(embedding.model_version.as_deref(), Some("0"));
    }
    db.close().await.unwrap();
}

#[tokio::test]
async fn oversized_vector_output_is_rejected_before_allocating_an_unbounded_matrix() {
    let directory = tempfile::tempdir().unwrap();
    let db = Database::open(directory.path(), Uuid::new_v4())
        .await
        .unwrap();
    db.configure_search_embedding_provider(Some(
        serde_json::from_value(json!({"kind":"token-hash","dimensions":65536})).unwrap(),
    ))
    .await
    .unwrap();
    let mut record = document();
    let chunk = record.chunks.take().unwrap().pop().unwrap();
    record.chunks = Some(
        (0..65)
            .map(|index| {
                let mut chunk = chunk.clone();
                chunk.id = format!("Note.md#{index}");
                chunk
            })
            .collect(),
    );
    db.upsert_search_document(record).await.unwrap();
    let stats = db.get_search_index_stats().await.unwrap();
    assert_eq!(stats.error_chunk_count, 65);
    assert_eq!(stats.ready_chunk_count, 0);
    let status = serde_json::to_value(db.get_search_embedding_runtime_status().unwrap()).unwrap();
    assert_eq!(status["phase"], "error");
    db.close().await.unwrap();
}
