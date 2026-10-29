use lapis_app_core::database::Database;
use lapis_app_protocol::{ErrorCode, database::call::DatabaseCall};
use serde_json::{Value, json};
use std::{collections::BTreeSet, future::Future, task::Poll};
use uuid::Uuid;

async fn call(db: &Database, method: &str, args: Vec<Value>) -> Value {
    let request = serde_json::from_value(json!({"method":method,"args":args})).unwrap();
    serde_json::to_value(db.call(request).await.unwrap()).unwrap()
}

fn value(value: Value) -> Value {
    json!({"kind":"value","value":value})
}

#[tokio::test]
async fn facade_keeps_missing_values_explicit_null_and_undefined_deletion_distinct() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    assert_eq!(
        call(&db, "getMeta", vec![value(json!("key"))]).await,
        json!({"kind":"undefined"})
    );
    call(
        &db,
        "setMeta",
        vec![value(json!("key")), value(Value::Null)],
    )
    .await;
    assert_eq!(
        call(&db, "getMeta", vec![value(json!("key"))]).await,
        value(Value::Null)
    );
    call(
        &db,
        "setMeta",
        vec![value(json!("key")), json!({"kind":"undefined"})],
    )
    .await;
    assert_eq!(
        call(&db, "getMeta", vec![value(json!("key"))]).await,
        json!({"kind":"undefined"})
    );
    db.close().await.unwrap();
}

fn numbers(value: Value) -> Value {
    match value {
        Value::Number(n) => json!(n.as_f64().unwrap()),
        Value::Array(values) => Value::Array(values.into_iter().map(numbers).collect()),
        Value::Object(fields) => {
            Value::Object(fields.into_iter().map(|(k, v)| (k, numbers(v))).collect())
        }
        value => value,
    }
}

#[tokio::test]
async fn every_allowlisted_call_retains_the_actual_turso_argument_and_result_reference() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/native-facade-contract.json"
    ))
    .unwrap();
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    let mut methods = BTreeSet::new();
    for case in fixture["cases"].as_array().unwrap() {
        let name = case["request"]["method"].as_str().unwrap();
        methods.insert(name.to_owned());
        let request = serde_json::from_value(case["request"].clone()).unwrap();
        let result = db
            .call(request)
            .await
            .unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
        let mut result = serde_json::to_value(result).unwrap();
        if name == "storeFileHistoryRevision" {
            let stored = &mut result["value"];
            assert!(!stored["fileId"].as_str().unwrap().is_empty());
            assert_eq!(stored["fileId"], stored["revision"]["fileId"]);
            assert!(
                !stored["revision"]["revisionId"]
                    .as_str()
                    .unwrap()
                    .is_empty()
            );
            stored["fileId"] = json!("<generated-file>");
            stored["revision"]["fileId"] = json!("<generated-file>");
            stored["revision"]["revisionId"] = json!("<generated-revision>");
        }
        assert_eq!(
            numbers(result),
            numbers(case["expected"].clone()),
            "{}",
            case["name"]
        );
    }
    assert_eq!(methods.len(), 68);
    db.close().await.unwrap();
}

#[test]
fn native_wire_methods_match_the_retained_allowlist_and_reject_paths_sql_and_unknown_values() {
    let source = include_str!("../../../packages/api/src/lib/storage/remote-app-database.ts");
    let methods = source
        .split_once("export const REMOTE_APP_DATABASE_METHODS = [")
        .unwrap()
        .1
        .split_once("] as const")
        .unwrap()
        .0;
    let mut count = 0;
    for name in methods.lines().filter_map(|line| {
        line.trim()
            .strip_prefix('"')
            .and_then(|s| s.split('"').next())
    }) {
        let call: DatabaseCall = serde_json::from_value(json!({"method":name})).unwrap();
        assert_eq!(serde_json::to_value(call).unwrap()["method"], json!(name));
        count += 1;
    }
    assert_eq!(count, 68);
    for request in [
        json!({"method":"execute","args":[]}),
        json!({"method":"open","args":[]}),
        json!({"method":"close","args":[]}),
        json!({"method":"getMeta","root":"/tmp","args":[]}),
        json!({"method":"getMeta","sql":"SELECT 1","args":[]}),
        json!({"method":"getMeta","args":[{"kind":"undefined","value":null}]}),
        json!({"method":"getMeta","args":[{"kind":"value"}]}),
    ] {
        assert!(
            serde_json::from_value::<DatabaseCall>(request.clone()).is_err(),
            "{request}"
        );
    }
}

#[tokio::test]
async fn malformed_or_oversized_calls_cannot_mutate_and_closed_calls_remain_closed() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    for request in [
        json!({"method":"setMeta","args":vec![json!({"kind":"undefined"});5]}),
        json!({"method":"setMeta","args":[value(json!(true)),value(json!("invalid"))]}),
        json!({"method":"renameIndexedFile","args":[value(json!("only-source.md"))]}),
        json!({"method":"setMeta","args":[value(json!("too-big")),value(json!("x".repeat(32*1024*1024)))]}),
    ] {
        let result = db.call(serde_json::from_value(request).unwrap()).await;
        assert_eq!(result.unwrap_err().code(), ErrorCode::InvalidRequest);
    }
    assert_eq!(db.revision().await.unwrap(), 0);
    db.close().await.unwrap();
    let result = db
        .call(serde_json::from_value(json!({"method":"getChangeRevision"})).unwrap())
        .await;
    assert_eq!(result.unwrap_err().code(), ErrorCode::Closed);
}

#[tokio::test]
async fn accepted_call_outlives_its_waiter_and_close_drains_its_publication() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    let request = serde_json::from_value(
        json!({"method":"setMeta","args":[value(json!("detached")),value(json!(true))]}),
    )
    .unwrap();
    let mut operation = Box::pin(db.call(request));
    std::future::poll_fn(|cx| {
        assert!(operation.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(operation);
    db.close().await.unwrap();
    let db = Database::open(profile.path(), id).await.unwrap();
    assert_eq!(
        call(&db, "getMeta", vec![value(json!("detached"))]).await,
        value(json!(true))
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn omitted_and_undefined_optional_read_queries_keep_defaults_without_writes() {
    let profile = tempfile::tempdir().unwrap();
    let db = Database::open(profile.path(), Uuid::new_v4())
        .await
        .unwrap();
    for method in [
        "queryMemoryCandidates",
        "listMemoryRecallSignals",
        "listMemoryJobs",
        "listIndexedFileManifest",
        "queryIndexedMetadata",
        "queryIndexedMetadataPage",
        "listSearchDocumentManifest",
    ] {
        assert_eq!(
            call(&db, method, vec![]).await,
            call(&db, method, vec![json!({"kind":"undefined"})]).await,
            "{method}"
        );
    }
    for method in ["searchDocuments", "searchDocumentPaths"] {
        assert_eq!(
            call(&db, method, vec![value(json!("absent"))]).await,
            call(
                &db,
                method,
                vec![value(json!("absent")), json!({"kind":"undefined"})]
            )
            .await
        );
    }
    assert_eq!(db.revision().await.unwrap(), 0);
    for args in [vec![], vec![json!({"kind":"undefined"})]] {
        let request = serde_json::from_value(json!({"method":"queryTasks","args":args})).unwrap();
        assert_eq!(
            db.call(request).await.unwrap_err().code(),
            ErrorCode::InvalidRequest
        );
    }
    let tasks: Value = serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/task-projection.json"
    ))
    .unwrap();
    call(
        &db,
        "upsertTaskProjection",
        vec![value(tasks["records"][0].clone())],
    )
    .await;
    let revision = db.revision().await.unwrap();
    assert_eq!(
        call(&db, "queryTasks", vec![]).await,
        call(&db, "queryTasks", vec![json!({"kind":"undefined"})]).await
    );
    assert_eq!(db.revision().await.unwrap(), revision);
    db.close().await.unwrap();
}
