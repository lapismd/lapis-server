use lapis_app_core::database::Database;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use uuid::Uuid;
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/memory-storage.json"
    ))
    .unwrap()
}
fn data<T: DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}

// The transport has JavaScript Number semantics. serde_json alone distinguishes
// integer 100 from f64 100.0; normalize that representation, retaining every key.
fn assert_json(left: impl Serialize, right: impl Serialize) {
    fn numbers(value: Value) -> Value {
        match value {
            Value::Number(n) => json!(n.as_f64().unwrap()),
            Value::Array(a) => Value::Array(a.into_iter().map(numbers).collect()),
            Value::Object(o) => {
                Value::Object(o.into_iter().map(|(k, v)| (k, numbers(v))).collect())
            }
            other => other,
        }
    }
    assert_eq!(
        numbers(serde_json::to_value(left).unwrap()),
        numbers(serde_json::to_value(right).unwrap())
    );
}

#[tokio::test]
async fn native_memory_records_keep_full_json_query_results_and_cold_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let vault = Uuid::new_v4();
    let db = Database::open(dir.path(), vault).await.unwrap();
    let f = fixture();
    db.upsert_memory_source_state(data(f["source"].clone()))
        .await
        .unwrap();
    for c in f["candidates"].as_array().unwrap() {
        db.upsert_memory_candidate(data(c.clone())).await.unwrap();
    }
    db.close().await.unwrap();
    let db = Database::open(dir.path(), vault).await.unwrap();
    assert_json(
        serde_json::to_value(db.get_memory_source_state("conversation-1").await.unwrap()).unwrap(),
        &f["source"],
    );
    assert_json(
        serde_json::to_value(db.list_memory_source_states().await.unwrap()).unwrap(),
        json!([f["source"]]),
    );
    for case in f["queries"]
        .as_array()
        .unwrap()
        .iter()
        .chain(f["tursoQueries"].as_array().unwrap())
    {
        let rows = serde_json::to_value(
            db.query_memory_candidates(data(case["query"].clone()))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            rows.as_array()
                .unwrap()
                .iter()
                .map(|r| r["candidate"]["id"].clone())
                .collect::<Vec<_>>(),
            *case["ids"].as_array().unwrap()
        );
        for row in rows.as_array().unwrap() {
            assert_json(
                row,
                f["candidates"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|c| c["candidate"]["id"] == row["candidate"]["id"])
                    .unwrap(),
            );
        }
    }
    db.delete_memory_candidates_by_conversation("conversation-2")
        .await
        .unwrap();
    let rows = serde_json::to_value(
        db.query_memory_candidates(Default::default())
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        rows.as_array()
            .unwrap()
            .iter()
            .map(|r| r["candidate"]["id"].clone())
            .collect::<Vec<_>>(),
        json!(["b", "c"]).as_array().unwrap().clone()
    );
    db.delete_memory_source_state("conversation-1")
        .await
        .unwrap();
    assert!(db.list_memory_source_states().await.unwrap().is_empty());
    db.close().await.unwrap();
}

#[tokio::test]
async fn native_memory_jobs_retain_the_shared_renewal_takeover_expiry_and_completed_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path(), Uuid::new_v4()).await.unwrap();
    let f = fixture();
    let mut changes = db.subscribe_to_changes();
    for step in f["jobSteps"].as_array().unwrap() {
        let before = db.revision().await.unwrap();
        let result = if step["action"] == "claim" {
            db.claim_memory_job(data(json!({"job":f["job"],"ownerId":step["ownerId"],"now":step["now"],"leaseMs":step["leaseMs"]}))).await.unwrap()
        } else {
            db.update_memory_job(data(json!({"jobId":f["job"]["id"],"ownerId":step["ownerId"],"now":step["now"],"patch":step["patch"]}))).await.unwrap()
        };
        assert_json(serde_json::to_value(&result).unwrap(), &step["expected"]);
        assert_eq!(
            db.revision().await.unwrap(),
            before + u64::from(result.is_some())
        );
        if result.is_some() {
            let event = changes.try_recv().unwrap();
            assert_eq!(event.revision, before + 1);
            assert_json(
                serde_json::to_value(event.domains).unwrap(),
                json!(["memory"]),
            );
        }
        assert!(changes.try_recv().is_err());
    }
    assert_json(
        serde_json::to_value(
            db.list_memory_jobs(Some("project:Projects/Alpha"))
                .await
                .unwrap(),
        )
        .unwrap(),
        json!([f["jobSteps"][8]["expected"]]),
    );
    assert!(db.list_memory_jobs(Some("other")).await.unwrap().is_empty());
    db.close().await.unwrap();
}

#[tokio::test]
async fn native_daily_recall_aggregation_and_clear_preserve_unrelated_domains() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path(), Uuid::new_v4()).await.unwrap();
    let f = fixture();
    db.set_meta("unrelated", Some(json!({"retained":true})))
        .await
        .unwrap();
    db.upsert_memory_source_state(data(f["source"].clone()))
        .await
        .unwrap();
    db.upsert_memory_candidate(data(f["candidates"][0].clone()))
        .await
        .unwrap();
    db.claim_memory_job(data(
        json!({"job":f["job"],"ownerId":"a","now":200,"leaseMs":100}),
    ))
    .await
    .unwrap();
    for (score, hits) in [(0.8, 2), (0.6, -4), (0.8, 3)] {
        db.record_memory_recall_signal(data(json!({"targetRef":"curated:memory:1","queryFingerprint":"salted-sha256","day":"2026-10-24","bestScore":score,"hitCount":hits}))).await.unwrap();
    }
    assert_json(
        serde_json::to_value(
            db.list_memory_recall_signals(Some("curated:memory:1"))
                .await
                .unwrap(),
        )
        .unwrap(),
        json!([{"targetRef":"curated:memory:1","queryFingerprint":"salted-sha256","day":"2026-10-24","bestScore":0.8,"hitCount":5}]),
    );
    assert!(
        db.list_memory_recall_signals(Some("other"))
            .await
            .unwrap()
            .is_empty()
    );
    db.clear_memory_derived_state().await.unwrap();
    assert!(db.list_memory_source_states().await.unwrap().is_empty());
    assert!(
        db.query_memory_candidates(Default::default())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        db.list_memory_recall_signals(None)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(db.list_memory_jobs(None).await.unwrap().is_empty());
    assert_eq!(
        db.get_meta("unrelated").await.unwrap(),
        Some(json!({"retained":true}))
    );
    db.close().await.unwrap();
}
