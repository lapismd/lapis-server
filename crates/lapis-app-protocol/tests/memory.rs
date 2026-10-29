use lapis_app_protocol::database::memory::{jobs::*, records::*};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/memory-storage.json"
    ))
    .unwrap()
}
fn roundtrip<T: DeserializeOwned + Serialize>(value: Value) {
    let typed: T = serde_json::from_value(value.clone()).unwrap();
    // Deserialize the emitted JSON again so JS Number integer/float spelling is
    // irrelevant; require exactly the source keys and their original values.
    fn same(a: &Value, b: &Value) {
        match (a, b) {
            (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64()),
            (Value::Array(a), Value::Array(b)) => {
                assert_eq!(a.len(), b.len());
                for (a, b) in a.iter().zip(b) {
                    same(a, b);
                }
            }
            (Value::Object(a), Value::Object(b)) => {
                assert_eq!(a.len(), b.len());
                for (k, v) in a {
                    same(v, b.get(k).unwrap());
                }
            }
            (a, b) => assert_eq!(a, b),
        }
    }
    same(&serde_json::to_value(typed).unwrap(), &value);
}

#[test]
fn memory_contract_retains_full_source_candidate_origin_and_job_fixture() {
    let f = fixture();
    roundtrip::<MemorySource>(f["source"].clone());
    for c in f["candidates"].as_array().unwrap() {
        roundtrip::<MemoryCandidateInput>(c.clone());
    }
    roundtrip::<MemoryJob>(f["job"].clone());
    for step in f["jobSteps"].as_array().unwrap() {
        if !step["expected"].is_null() {
            roundtrip::<MemoryJob>(step["expected"].clone());
        }
        if step["action"] == "claim" {
            roundtrip::<MemoryJobClaim>(
                json!({"job":f["job"],"ownerId":step["ownerId"],"now":step["now"],"leaseMs":step["leaseMs"]}),
            );
        } else {
            roundtrip::<MemoryJobUpdate>(
                json!({"jobId":f["job"]["id"],"ownerId":step["ownerId"],"now":step["now"],"patch":step["patch"]}),
            );
        }
    }
}

#[test]
fn memory_dtos_reject_raw_queries_unknown_patch_authority_and_invalid_enum_values() {
    let mut candidate = fixture()["candidates"][0].clone();
    candidate["candidate"]["importance"] = json!(4.0);
    assert_eq!(
        serde_json::from_value::<MemoryCandidateInput>(candidate.clone())
            .unwrap()
            .candidate
            .importance,
        4
    );
    for bad in [0.0, 6.0, 4.5] {
        candidate["candidate"]["importance"] = json!(bad);
        assert!(serde_json::from_value::<MemoryCandidateInput>(candidate.clone()).is_err());
    }
    for bad in [
        json!({"scopeKind":"private"}),
        json!({"sql":"SELECT * FROM memory_jobs"}),
        json!({"states":["running"]}),
    ] {
        assert!(serde_json::from_value::<MemoryCandidateQuery>(bad).is_err());
    }
    assert!(serde_json::from_value::<MemoryRecallSignal>(json!({"targetRef":"x","queryFingerprint":"salted","day":"today","bestScore":0.5,"hitCount":1,"query":"raw sensitive query"})).is_err());
    assert!(
        serde_json::from_value::<MemoryJobUpdate>(
            json!({"jobId":"x","ownerId":"a","now":1,"patch":{"status":"completed","ownerId":"b"}})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<MemoryJobUpdate>(
            json!({"jobId":"x","ownerId":"a","now":1,"patch":{"status":"cancelled"}})
        )
        .is_err()
    );
}
