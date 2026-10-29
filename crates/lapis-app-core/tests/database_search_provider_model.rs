#[path = "support/embedding_reference.rs"]
mod embedding_reference;

use lapis_app_core::{
    database::Database,
    embedding::host::{NativeEmbeddingHost, OnnxModelAssets},
};
use lapis_app_protocol::database::search::{
    embedding::{SearchEmbeddingProviderConfig, SearchEmbeddingRuntimePhase},
    records::SearchDocumentRecord,
};
use serde_json::json;
use std::{future::Future, path::PathBuf, task::Poll};
use uuid::Uuid;

#[tokio::test]
#[ignore = "Requires qualified native assets; run pnpm test:search:onnx"]
async fn admitted_real_model_publishes_after_detach_and_preserves_cold_configuration() {
    let models = PathBuf::from(std::env::var_os("LAPIS_TEST_ONNX_MODEL").unwrap());
    let fixture = embedding_reference::qualified_fixture();
    let host = NativeEmbeddingHost::new(
        PathBuf::from(std::env::var_os("LAPIS_TEST_ONNX_RUNTIME").unwrap()),
        vec![OnnxModelAssets {
            model_id: fixture["modelId"].as_str().unwrap().into(),
            revision: fixture["revision"].as_str().unwrap().into(),
            dtype: "q8".into(),
            model: models.join("onnx/model_quantized.onnx"),
            tokenizer: models.join("tokenizer.json"),
            tokenizer_config: models.join("tokenizer_config.json"),
        }],
    );
    let directory = tempfile::tempdir().unwrap();
    let vault = Uuid::new_v4();
    let db = Database::open_with_embeddings(directory.path(), vault, host.clone())
        .await
        .unwrap();
    let configuration = |dimensions| {
        serde_json::from_value::<SearchEmbeddingProviderConfig>(json!({"kind":"transformers-js","modelId":fixture["modelId"],"revision":fixture["revision"],"dtype":"q8","dimensions":dimensions,"normalize":true})).unwrap()
    };
    db.configure_search_embedding_provider(Some(configuration(384)))
        .await
        .unwrap();
    assert_eq!(
        db.get_search_embedding_runtime_status()
            .unwrap()
            .unwrap()
            .phase,
        SearchEmbeddingRuntimePhase::Downloading
    );
    let rows = fixture["queries"].as_array().unwrap();
    for (index, row) in rows.iter().enumerate() {
        let record = document(index, row["text"].as_str().unwrap());
        if index + 1 == rows.len() {
            let mut changes = db.subscribe_to_changes();
            let mut pending = Box::pin(db.upsert_search_document(record));
            std::future::poll_fn(|context| {
                assert!(pending.as_mut().poll(context).is_pending());
                Poll::Ready(())
            })
            .await;
            drop(pending);
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                loop {
                    if db
                        .get_search_embedding_runtime_status()
                        .unwrap()
                        .unwrap()
                        .phase
                        == SearchEmbeddingRuntimePhase::Embedding
                    {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("status stays observable while the actor owns inference");
            let event = changes.recv().await.unwrap();
            assert_eq!(event.paths, vec![format!("{index}.md")]);
        } else {
            db.upsert_search_document(record).await.unwrap();
        }
        let record = db
            .get_search_document(&format!("{index}.md"))
            .await
            .unwrap()
            .unwrap();
        let embedding = record.chunks.unwrap().pop().unwrap().embedding.unwrap();
        let vector = embedding.vector.unwrap();
        assert_eq!(vector.len(), 384);
        for (dimension, (actual, expected)) in vector
            .iter()
            .zip(row["vector"].as_array().unwrap())
            .enumerate()
        {
            assert!(
                (actual - expected.as_f64().unwrap()).abs() < 0.00001,
                "model case {index}, dimension {dimension}: actual {actual}, expected {expected}"
            );
        }
        assert_eq!(
            embedding.fingerprint.as_deref(),
            fixture["chunks"][index]["fingerprint"].as_str()
        );
    }
    assert_eq!(
        db.get_search_index_stats().await.unwrap().ready_chunk_count,
        6
    );
    db.configure_search_embedding_provider(Some(configuration(768)))
        .await
        .unwrap();
    assert_eq!(
        db.get_search_index_stats()
            .await
            .unwrap()
            .pending_chunk_count,
        6
    );
    db.upsert_search_document(document(0, rows[0]["text"].as_str().unwrap()))
        .await
        .unwrap();
    let row = serde_json::to_value(db.get_search_document("0.md").await.unwrap()).unwrap();
    assert_eq!(row["chunks"][0]["embedding"]["status"], "error");
    assert!(row["chunks"][0]["embedding"].get("vector").is_none());
    db.configure_search_embedding_provider(Some(configuration(384)))
        .await
        .unwrap();
    db.upsert_search_document(document(0, rows[0]["text"].as_str().unwrap()))
        .await
        .unwrap();
    db.close().await.unwrap();
    let db = Database::open_with_embeddings(directory.path(), vault, host)
        .await
        .unwrap();
    assert_eq!(
        db.get_search_embedding_runtime_status()
            .unwrap()
            .unwrap()
            .phase,
        SearchEmbeddingRuntimePhase::Downloading
    );
    assert_eq!(
        db.get_search_index_stats().await.unwrap().ready_chunk_count,
        1
    );
    db.upsert_search_document(document(1, rows[1]["text"].as_str().unwrap()))
        .await
        .unwrap();
    assert_eq!(
        db.get_search_index_stats().await.unwrap().ready_chunk_count,
        2
    );
    db.close().await.unwrap();
}
fn document(index: usize, text: &str) -> SearchDocumentRecord {
    serde_json::from_value(json!({"path":format!("{index}.md"),"name":"Fixture","extension":"md","checksum":"fixture","content":text,"tags":[],"tagParts":[],"tagHierarchy":[],"chunks":[{"id":format!("{index}.md#0"),"text":text,"startOffset":0,"endOffset":text.encode_utf16().count()}]})).unwrap()
}
