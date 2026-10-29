use lapis_app_core::database::Database;
use lapis_app_protocol::{ErrorCode, database::search::records::SearchDocumentRecord};
use serde_json::json;
use uuid::Uuid;

fn document(path: &str, content: &str) -> SearchDocumentRecord {
    serde_json::from_value(json!({"path":path,"name":path,"content":content,"extension":"md","checksum":"fixture","sourceProviderId":"markdown","tags":[],"tagParts":[],"tagHierarchy":[],"chunks":[{"id":format!("{path}#0"),"text":content,"startOffset":0,"endOffset":content.encode_utf16().count()}]})).unwrap()
}
async fn configured() -> (tempfile::TempDir, Database) {
    let root = tempfile::tempdir().unwrap();
    let db = Database::open(root.path(), Uuid::new_v4()).await.unwrap();
    db.configure_search_embedding_provider(Some(
        serde_json::from_value(json!({"kind":"token-hash","dimensions":48})).unwrap(),
    ))
    .await
    .unwrap();
    (root, db)
}
#[tokio::test]
async fn scoped_semantic_ranking_evaluates_every_document_before_the_final_limit() {
    let (_root, db) = configured().await;
    for n in 0..111 {
        let path = if n == 110 {
            "Projects/Z-winner.md".into()
        } else {
            format!("Projects/{n:03}.md")
        };
        db.upsert_search_document(document(
            &path,
            if n == 110 {
                "unique ocean needle"
            } else {
                "mundane filler"
            },
        ))
        .await
        .unwrap();
    }
    db.upsert_search_document(document("Projects-other/Outside.md", "unique ocean needle"))
        .await
        .unwrap();
    for mode in ["vector", "hybrid", "auto"] {
        let options = json!({"mode":mode,"pathPrefix":"Projects","sourceProviderIds":["markdown"],"limit":1,"includeDiagnostics":true});
        let rows = db
            .search_documents(
                "unique ocean needle",
                serde_json::from_value(options.clone()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        let value = serde_json::to_value(&rows).unwrap();
        assert_eq!(value[0]["document"]["path"], "Projects/Z-winner.md");
        assert_eq!(value[0]["diagnostics"]["lexicalCandidateCount"], 111);
        assert_eq!(
            db.search_document_paths(
                "unique ocean needle",
                serde_json::from_value(options).unwrap()
            )
            .await
            .unwrap(),
            ["Projects/Z-winner.md"]
        );
    }
    db.close().await.unwrap();
}
#[tokio::test]
async fn missing_model_retains_lexical_degradation_without_hash_vectors_or_writes() {
    let (_root, db) = configured().await;
    db.upsert_search_document(document("Ocean.md", "ocean water"))
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
    let revision = db.revision().await.unwrap();
    let rows = db
        .search_documents(
            "ocean",
            serde_json::from_value(json!({"mode":"hybrid","includeDiagnostics":true})).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let value = serde_json::to_value(rows).unwrap();
    assert_eq!(value[0]["retrievalMode"], "lexical");
    assert_eq!(value[0]["diagnostics"]["vectorCandidateCount"], 0);
    assert!(value[0]["scoreBreakdown"].get("vector").is_none());
    let status = serde_json::to_value(db.get_search_embedding_runtime_status().unwrap()).unwrap();
    assert_eq!(status["phase"], "error");
    assert_eq!(db.revision().await.unwrap(), revision);
    let doc = serde_json::to_value(db.get_search_document("Ocean.md").await.unwrap()).unwrap();
    assert_eq!(doc["chunks"][0]["embedding"]["status"], "pending");
    assert!(doc["chunks"][0]["embedding"].get("vector").is_none());
    assert!(
        db.search_document_paths(
            "ocean",
            serde_json::from_value(json!({"mode":"vector"})).unwrap()
        )
        .await
        .unwrap()
        .is_empty()
    );
    // Structured queries retain complete-table lexical evaluation if inference is unavailable.
    assert_eq!(
        db.search_document_paths(
            "file:Ocean",
            serde_json::from_value(json!({"mode":"vector"})).unwrap()
        )
        .await
        .unwrap(),
        ["Ocean.md"]
    );
    db.close().await.unwrap();
}
#[tokio::test]
async fn only_ready_matching_vectors_score_and_only_three_previews_are_selected() {
    let (_root, db) = configured().await;
    db.upsert_search_document(document("Seed.md", "ocean"))
        .await
        .unwrap();
    let seed = db.get_search_document("Seed.md").await.unwrap().unwrap();
    let ready = seed.chunks.unwrap()[0].embedding.clone().unwrap();
    db.delete_search_document("Seed.md").await.unwrap();
    let mut record = document("Unrelated.md", "no query terms");
    record.chunks=Some((0..6).map(|n|{
        let mut chunk=document("Unused.md","unrelated preview").chunks.unwrap().pop().unwrap();
        chunk.id=format!("Unrelated.md#{n}");chunk.start_offset=(n*20) as f64;chunk.end_offset=(n*20+17) as f64;
        chunk.embedding=Some(ready.clone());
        if n==4 {chunk.embedding.as_mut().unwrap().status=lapis_app_protocol::database::search::records::SearchEmbeddingStatus::Pending;}
        if n==5 {chunk.embedding.as_mut().unwrap().vector=Some(vec![1.]);}
        chunk
    }).collect());
    db.upsert_prepared_search_document(record).await.unwrap();
    let rows = db
        .search_documents(
            "ocean",
            serde_json::from_value(
                json!({"mode":"vector","pathPrefix":"Unrelated.md","snippetLength":9}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].matched_chunk_ids,
        ["Unrelated.md#0", "Unrelated.md#1", "Unrelated.md#2"]
    );
    assert_eq!(rows[0].snippets.len(), 3);
    for (n, snippet) in rows[0].snippets.iter().enumerate() {
        assert_eq!(snippet.text, "unrelated");
        assert_eq!(snippet.offset, (n * 20) as f64);
        assert!(snippet.ranges.is_empty());
    }
    db.close().await.unwrap();
}
#[tokio::test]
async fn unconfigured_modes_and_closed_admission_retain_the_existing_turso_contract() {
    let root = tempfile::tempdir().unwrap();
    let db = Database::open(root.path(), Uuid::new_v4()).await.unwrap();
    db.upsert_search_document(document("Ocean.md", "ocean"))
        .await
        .unwrap();
    for mode in ["auto", "hybrid", "lexical"] {
        assert_eq!(
            db.search_document_paths(
                "ocean",
                serde_json::from_value(json!({"mode":mode})).unwrap()
            )
            .await
            .unwrap(),
            ["Ocean.md"]
        );
    }
    assert!(
        db.search_document_paths(
            "ocean",
            serde_json::from_value(json!({"mode":"vector"})).unwrap()
        )
        .await
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        db.search_document_paths(
            "file:Ocean",
            serde_json::from_value(json!({"mode":"vector"})).unwrap()
        )
        .await
        .unwrap(),
        ["Ocean.md"]
    );
    db.close().await.unwrap();
    assert_eq!(
        db.search_documents(
            "ocean",
            serde_json::from_value(json!({"mode":"vector"})).unwrap()
        )
        .await
        .unwrap_err()
        .code(),
        ErrorCode::Closed
    );
}

#[tokio::test]
async fn native_candidate_batches_preserve_every_selected_path_and_rank() {
    let (_root, db) = configured().await;
    let expected = (0..300)
        .map(|n| format!("Notes/{n:03}.md"))
        .collect::<Vec<_>>();
    for path in &expected {
        db.upsert_search_document(document(path, "ocean"))
            .await
            .unwrap();
    }
    for mode in ["lexical", "vector", "hybrid"] {
        let options = json!({"mode":mode,"limit":300,"includeDiagnostics":true});
        let rows = db
            .search_documents("ocean", serde_json::from_value(options.clone()).unwrap())
            .await
            .unwrap();
        assert_eq!(rows.len(), 300);
        assert_eq!(
            rows.iter()
                .map(|r| r.document.path.clone())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            rows[0]
                .diagnostics
                .as_ref()
                .unwrap()
                .lexical_candidate_count,
            300
        );
        assert_eq!(
            db.search_document_paths("ocean", serde_json::from_value(options).unwrap())
                .await
                .unwrap(),
            expected
        );
    }
    db.close().await.unwrap();
}
