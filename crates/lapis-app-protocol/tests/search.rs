use lapis_app_protocol::database::search::{records::*, *};
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/search-storage.json"
    ))
    .unwrap()
}
fn normalized(v: Value) -> Value {
    match v {
        Value::Number(n) => json!(n.as_f64().unwrap()),
        Value::Array(v) => Value::Array(v.into_iter().map(normalized).collect()),
        Value::Object(v) => Value::Object(v.into_iter().map(|(k, v)| (k, normalized(v))).collect()),
        v => v,
    }
}
#[test]
fn search_dtos_retain_complete_prepared_documents_and_explicit_null_frontmatter() {
    for record in fixture()["records"].as_array().unwrap() {
        let typed: SearchDocumentRecord = serde_json::from_value(record["stored"].clone()).unwrap();
        assert_eq!(
            normalized(serde_json::to_value(typed).unwrap()),
            normalized(record["stored"].clone())
        );
    }
    let page: SearchManifestPage =
        serde_json::from_value(json!({"rows":fixture()["manifest"]})).unwrap();
    assert_eq!(
        normalized(serde_json::to_value(page).unwrap()),
        normalized(json!({"rows":fixture()["manifest"]}))
    );
    let stats: SearchIndexStats = serde_json::from_value(fixture()["stats"].clone()).unwrap();
    assert_eq!(serde_json::to_value(stats).unwrap(), fixture()["stats"]);
}
#[test]
fn search_dtos_reject_native_paths_sql_and_unknown_nested_fields() {
    let base = fixture()["records"][0]["stored"].clone();
    let mut bad = base.clone();
    bad["databasePath"] = json!("/tmp/other.db");
    assert!(serde_json::from_value::<SearchDocumentRecord>(bad).is_err());
    let mut bad = base.clone();
    bad["chunks"][0]["embedding"]["ownerId"] = json!("arbitrary");
    assert!(serde_json::from_value::<SearchDocumentRecord>(bad).is_err());
    let mut bad = base;
    bad["chunks"][0]["kind"] = json!("unknown");
    assert!(serde_json::from_value::<SearchDocumentRecord>(bad).is_err());
    for query in [
        json!({"sql":"SELECT * FROM search_docs"}),
        json!({"limit":1.5}),
    ] {
        assert!(serde_json::from_value::<SearchManifestQuery>(query).is_err());
    }
}

#[test]
fn retrieval_dtos_retain_full_results_and_reject_invalid_options() {
    use lapis_app_protocol::database::search::retrieval::{SearchOptions, SearchResult};
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/search-retrieval.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let options: SearchOptions = serde_json::from_value(case["options"].clone()).unwrap();
        assert_eq!(serde_json::to_value(options).unwrap(), case["options"]);
        let results: Vec<SearchResult> = serde_json::from_value(case["turso"].clone()).unwrap();
        assert_eq!(
            normalized(serde_json::to_value(results).unwrap()),
            normalized(case["turso"].clone())
        );
    }
    for bad in [
        json!({"sql":"SELECT *"}),
        json!({"databasePath":"/tmp/other.db"}),
        json!({"mode":"execute"}),
        json!({"limit":-1}),
        json!({"snippetLength":1.5}),
    ] {
        assert!(serde_json::from_value::<SearchOptions>(bad).is_err())
    }
}
