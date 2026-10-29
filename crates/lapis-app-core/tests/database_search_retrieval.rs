mod search_support;
use lapis_app_core::database::Database;
use lapis_app_protocol::ErrorCode;
use search_support::assert_json;
use serde_json::{Value, json};
use uuid::Uuid;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/search-retrieval.json"
    ))
    .unwrap()
}

#[tokio::test]
async fn configured_semantic_search_is_not_silently_replaced_with_lexical_results() {
    let directory = tempfile::tempdir().unwrap();
    let db = Database::open(directory.path(), Uuid::new_v4())
        .await
        .unwrap();
    db.upsert_search_document(
        serde_json::from_value(fixture()["records"][0]["document"].clone()).unwrap(),
    )
    .await
    .unwrap();
    db.set_meta(
        "search.embeddingProvider",
        Some(json!({"kind":"local","modelId":"fixture/unqualified"})),
    )
    .await
    .unwrap();
    let revision = db.revision().await.unwrap();
    assert_eq!(
        db.search_documents("Alpha", Default::default())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Unsupported
    );
    assert_eq!(
        db.search_document_paths(
            "file:Daily",
            serde_json::from_value(json!({"mode":"lexical"})).unwrap()
        )
        .await
        .unwrap(),
        ["Projects/Daily.md"]
    );
    assert_eq!(db.revision().await.unwrap(), revision);
    db.close().await.unwrap();
}

#[tokio::test]
async fn table_queries_rank_all_eligible_documents_after_cold_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let vault = Uuid::new_v4();
    let db = Database::open(directory.path(), vault).await.unwrap();
    for index in 0..111 {
        let winner = index == 110;
        let document = json!({"path":if winner{"Projects/Z-winner.md".to_owned()}else{format!("Projects/{index:03}.md")},"name":if winner{"needle"}else{"filler"},"content":if winner{"winning note"}else{"needle filler"},"metadataText":if winner{"{\"status\":\"ready\"}"}else{"{}"},"sourceProviderId":"markdown","extension":"md","checksum":"fixture","tags":[],"tagParts":[],"tagHierarchy":[]});
        db.upsert_search_document(serde_json::from_value(document).unwrap())
            .await
            .unwrap();
    }
    db.close().await.unwrap();
    let db = Database::open(directory.path(), vault).await.unwrap();
    let revision = db.revision().await.unwrap();
    for (query, options) in [
        ("file:needle", json!({})),
        ("[status]:ready", json!({})),
        ("[status]", json!({})),
        ("-filler", json!({})),
        ("needle", json!({"caseSensitive":true})),
        ("needle", json!({"pathPrefix":"Projects"})),
        ("needle", json!({"sourceProviderIds":["markdown"]})),
    ] {
        let mut options = options;
        options["mode"] = json!("lexical");
        options["limit"] = json!(1);
        assert_eq!(
            db.search_document_paths(query, serde_json::from_value(options).unwrap())
                .await
                .unwrap(),
            ["Projects/Z-winner.md"]
        );
    }
    assert_eq!(db.revision().await.unwrap(), revision);
    db.close().await.unwrap();
}

#[tokio::test]
async fn invalid_or_unconfigured_queries_preserve_the_owner_and_closed_admission() {
    let directory = tempfile::tempdir().unwrap();
    let db = Database::open(directory.path(), Uuid::new_v4())
        .await
        .unwrap();
    db.upsert_search_document(
        serde_json::from_value(fixture()["records"][0]["document"].clone()).unwrap(),
    )
    .await
    .unwrap();
    let revision = db.revision().await.unwrap();
    for options in [
        json!({"limit":100001}),
        json!({"snippetLength":33554433}),
        json!({"sourceProviderIds":vec!["x";401]}),
    ] {
        assert_eq!(
            db.search_documents("Alpha", serde_json::from_value(options).unwrap())
                .await
                .unwrap_err()
                .code(),
            ErrorCode::InvalidRequest
        );
    }
    for query in [
        "a".repeat(65537),
        "a".repeat(32769),
        "a ".repeat(4097),
        format!("{}a{}", "(".repeat(129), ")".repeat(129)),
    ] {
        assert_eq!(
            db.search_documents(&query, Default::default())
                .await
                .unwrap_err()
                .code(),
            ErrorCode::InvalidRequest
        );
    }
    for (mode, expected) in [("vector", vec![]), ("hybrid", vec!["Projects/Daily.md"])] {
        assert_eq!(
            db.search_document_paths(
                "Alpha",
                serde_json::from_value(json!({"mode":mode})).unwrap()
            )
            .await
            .unwrap(),
            expected
        );
    }
    assert_eq!(
        db.search_document_paths("file:Daily", Default::default())
            .await
            .unwrap(),
        ["Projects/Daily.md"]
    );
    assert_eq!(db.revision().await.unwrap(), revision);
    db.close().await.unwrap();
    assert_eq!(
        db.search_documents("Alpha", Default::default())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Closed
    );
    assert_eq!(
        db.search_document_paths("Alpha", Default::default())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Closed
    );
}
#[tokio::test]
async fn native_lexical_and_structured_results_retain_the_complete_turso_reference() {
    let directory = tempfile::tempdir().unwrap();
    let db = Database::open(directory.path(), Uuid::new_v4())
        .await
        .unwrap();
    for entry in fixture()["records"].as_array().unwrap() {
        let document = &entry["document"];
        let path = document["path"].as_str().unwrap();
        let properties=entry["properties"].as_object().unwrap().iter().map(|(name,value)|json!({"path":path,"name":name,"value":value,"inferredType":"fixture"})).collect::<Vec<_>>();
        db.upsert_indexed_metadata(serde_json::from_value(json!({"file":{"path":path,"normalizedPath":path,"extension":"md","mtime":1,"size":1,"hash":"reference","indexed":true},"metadata":{"path":path,"hash":"reference","parserVersion":"fixture","metadata":{}},"links":[],"tags":[],"properties":properties})).unwrap()).await.unwrap();
        db.upsert_search_document(serde_json::from_value(document.clone()).unwrap())
            .await
            .unwrap();
    }
    let revision = db.revision().await.unwrap();
    for case in fixture()["cases"].as_array().unwrap() {
        eprintln!("retrieval reference: {}", case["name"]);
        let results = db
            .search_documents(
                case["query"].as_str().unwrap(),
                serde_json::from_value(case["options"].clone()).unwrap(),
            )
            .await
            .unwrap();
        assert_json(
            serde_json::to_value(&results).unwrap(),
            case["turso"].clone(),
        );
        let paths = db
            .search_document_paths(
                case["query"].as_str().unwrap(),
                serde_json::from_value(case["options"].clone()).unwrap(),
            )
            .await
            .unwrap();
        assert_json(
            serde_json::to_value(paths).unwrap(),
            Value::Array(
                case["turso"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r["document"]["path"].clone())
                    .collect(),
            ),
        );
    }
    assert_eq!(db.revision().await.unwrap(), revision);
    db.close().await.unwrap();
}
