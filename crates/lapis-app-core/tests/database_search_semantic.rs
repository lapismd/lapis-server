use lapis_app_core::database::Database;
use serde_json::Value;
use uuid::Uuid;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/search-semantic-retrieval.json"
    ))
    .unwrap()
}
fn assert_result(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-12,
            "{actual} != {expected}"
        ),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                assert_result(a, b)
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{actual} != {expected}");
            for (key, value) in b {
                assert_result(a.get(key).unwrap_or_else(|| panic!("missing {key}")), value)
            }
        }
        _ => assert_eq!(actual, expected),
    }
}
#[tokio::test]
async fn native_semantic_results_retain_the_turso_reference_after_cold_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let vault = Uuid::new_v4();
    let db = Database::open(directory.path(), vault).await.unwrap();
    let reference = fixture();
    db.configure_search_embedding_provider(Some(
        serde_json::from_value(reference["config"].clone()).unwrap(),
    ))
    .await
    .unwrap();
    for record in reference["records"].as_array().unwrap() {
        db.upsert_prepared_search_document(serde_json::from_value(record.clone()).unwrap())
            .await
            .unwrap();
    }
    db.close().await.unwrap();
    let db = Database::open(directory.path(), vault).await.unwrap();
    let revision = db.revision().await.unwrap();
    for case in reference["cases"].as_array().unwrap() {
        eprintln!("semantic reference: {}", case["name"]);
        let query = case["query"].as_str().unwrap();
        let options = serde_json::from_value(case["options"].clone()).unwrap();
        let mut result =
            serde_json::to_value(db.search_documents(query, options).await.unwrap()).unwrap();
        for row in result.as_array_mut().unwrap() {
            row["document"] = row["document"]["path"].clone();
        }
        assert_result(&result, &case["turso"]);
        let paths = db
            .search_document_paths(
                query,
                serde_json::from_value(case["options"].clone()).unwrap(),
            )
            .await
            .unwrap();
        let expected: Vec<_> = case["turso"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["document"].as_str().unwrap())
            .collect();
        assert_eq!(paths, expected);
    }
    assert_eq!(db.revision().await.unwrap(), revision);
    db.close().await.unwrap();
}

#[tokio::test]
async fn selected_semantic_queries_retain_enhancement_signals_before_final_limits() {
    let directory = tempfile::tempdir().unwrap();
    let db = Database::open(directory.path(), Uuid::new_v4())
        .await
        .unwrap();
    let reference = fixture();
    db.configure_search_embedding_provider(Some(
        serde_json::from_value(reference["config"].clone()).unwrap(),
    ))
    .await
    .unwrap();
    for record in reference["records"].as_array().unwrap() {
        db.upsert_prepared_search_document(serde_json::from_value(record.clone()).unwrap())
            .await
            .unwrap();
    }
    let revision = db.revision().await.unwrap();
    for entry in reference["enhancementCases"].as_array().unwrap() {
        let base = &reference["cases"][entry["caseIndex"].as_u64().unwrap() as usize];
        let mut options = base["options"].clone();
        options["queryEnhancement"] = entry["request"].clone();
        if entry.get("limit").is_some() {
            options["limit"] = entry["limit"].clone();
        }
        let request = serde_json::from_value(serde_json::json!({"method":"searchDocuments","args":[{"kind":"value","value":base["query"]},{"kind":"value","value":options}]})).unwrap();
        let result = serde_json::to_value(db.call(request).await.unwrap()).unwrap();
        let mut actual = result["value"].clone();
        for row in actual.as_array_mut().unwrap() {
            row["document"] = row["document"]["path"].clone();
        }
        let mut expected = base["turso"].clone();
        if let Some(limit) = entry["limit"].as_u64() {
            expected.as_array_mut().unwrap().truncate(limit as usize);
        }
        for row in expected.as_array_mut().unwrap() {
            row["diagnostics"]["queryEnhancement"] = entry["expected"].clone();
        }
        assert_result(&actual, &expected);
    }
    assert_eq!(db.revision().await.unwrap(), revision);
    db.close().await.unwrap();
}
