#[path = "support/embedding_reference.rs"]
mod embedding_reference;

use lapis_app_core::{
    database::Database,
    embedding::host::{NativeEmbeddingHost, OnnxModelAssets},
};
use lapis_app_protocol::database::search::{
    embedding::SearchEmbeddingRuntimePhase, records::SearchDocumentRecord,
};
use serde_json::json;
use std::{future::Future, path::PathBuf, task::Poll};
use uuid::Uuid;

#[tokio::test]
#[ignore = "Requires qualified native assets; run pnpm test:search:onnx"]
async fn admitted_onnx_queries_retain_model_ranks_after_reopen_and_detach() {
    let fixture = embedding_reference::qualified_fixture();
    let folder = PathBuf::from(std::env::var_os("LAPIS_TEST_ONNX_MODEL").unwrap());
    let host = NativeEmbeddingHost::new(
        PathBuf::from(std::env::var_os("LAPIS_TEST_ONNX_RUNTIME").unwrap()),
        vec![OnnxModelAssets {
            model_id: fixture["modelId"].as_str().unwrap().into(),
            revision: fixture["revision"].as_str().unwrap().into(),
            dtype: "q8".into(),
            model: folder.join("onnx/model_quantized.onnx"),
            tokenizer: folder.join("tokenizer.json"),
            tokenizer_config: folder.join("tokenizer_config.json"),
        }],
    );
    let root = tempfile::tempdir().unwrap();
    let vault = Uuid::new_v4();
    let db = Database::open_with_embeddings(root.path(), vault, host.clone())
        .await
        .unwrap();
    db.configure_search_embedding_provider(Some(serde_json::from_value(json!({"kind":"transformers-js","modelId":fixture["modelId"],"revision":fixture["revision"],"dtype":"q8","dimensions":384,"normalize":true})).unwrap())).await.unwrap();
    let rows = fixture["queries"].as_array().unwrap();
    for (index, row) in rows.iter().enumerate() {
        let doc:SearchDocumentRecord=serde_json::from_value(json!({"path":format!("{index}.md"),"name":format!("Record {index}"),"content":row["text"],"extension":"md","checksum":"model-reference","tags":[],"tagParts":[],"tagHierarchy":[],"chunks":[{"id":format!("{index}.md#0"),"text":row["text"],"startOffset":0,"endOffset":row["text"].as_str().unwrap().encode_utf16().count(),"embedding":{"status":"ready","modelId":fixture["modelId"],"vector":row["vector"],"dimensions":384}}]})).unwrap();
        db.upsert_prepared_search_document(doc).await.unwrap();
    }
    db.close().await.unwrap();
    let db = Database::open_with_embeddings(root.path(), vault, host.clone())
        .await
        .unwrap();
    let revision = db.revision().await.unwrap();
    for row in rows {
        let query = row["text"].as_str().unwrap();
        let expected_query = row["vector"].as_array().unwrap();
        let mut expected = rows
            .iter()
            .enumerate()
            .map(|(index, doc)| {
                let score = doc["vector"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .zip(expected_query)
                    .map(|(a, b)| a.as_f64().unwrap() * b.as_f64().unwrap())
                    .sum::<f64>();
                (index, score)
            })
            .filter(|(_, score)| *score >= 0.2)
            .collect::<Vec<_>>();
        expected.sort_by(|a, b| b.1.total_cmp(&a.1));
        let results = db
            .search_documents(
                query,
                serde_json::from_value(json!({"mode":"vector","includeDiagnostics":true})).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(results.len(), expected.len());
        for (result, (index, score)) in results.iter().zip(&expected) {
            assert_eq!(result.document.path, format!("{index}.md"));
            assert!(
                (result.score_breakdown.vector.unwrap() - score).abs() < 0.0002,
                "actual model query must retain the qualified embedding tolerance"
            );
            assert_eq!(result.matched_chunk_ids, [format!("{index}.md#0")]);
            assert!(!result.snippets.is_empty());
            let diagnostics = serde_json::to_value(result.diagnostics.as_ref().unwrap()).unwrap();
            assert_eq!(diagnostics["providerKind"], "transformers-js");
            assert_eq!(diagnostics["appliedMode"], "vector");
            assert_eq!(diagnostics["vectorCandidateCount"], expected.len());
        }
    }
    assert_eq!(db.revision().await.unwrap(), revision);
    db.close().await.unwrap();
    let db = Database::open_with_embeddings(root.path(), vault, host.clone())
        .await
        .unwrap();
    assert_eq!(
        db.get_search_embedding_runtime_status()
            .unwrap()
            .unwrap()
            .phase,
        SearchEmbeddingRuntimePhase::Downloading
    );
    let options = serde_json::from_value(json!({"mode":"hybrid"})).unwrap();
    let mut query = Box::pin(db.search_documents(rows[0]["text"].as_str().unwrap(), options));
    std::future::poll_fn(|cx| {
        assert!(query.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(query);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if db
                .get_search_embedding_runtime_status()
                .unwrap()
                .unwrap()
                .phase
                == SearchEmbeddingRuntimePhase::Ready
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("detaching the waiter must not cancel the admitted model query");
    db.close().await.unwrap();
    let db = Database::open_with_embeddings(root.path(), vault, host)
        .await
        .unwrap();
    assert_eq!(db.revision().await.unwrap(), revision);
    assert_eq!(
        db.get_search_embedding_runtime_status()
            .unwrap()
            .unwrap()
            .phase,
        SearchEmbeddingRuntimePhase::Downloading
    );
    assert_eq!(
        db.get_search_index_stats().await.unwrap().ready_chunk_count,
        6
    );
    db.close().await.unwrap();
}
