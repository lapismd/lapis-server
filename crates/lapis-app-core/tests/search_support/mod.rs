#![allow(dead_code)]
use lapis_app_core::database::Database;
use serde_json::Value;

pub fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../packages/api/src/lib/__tests__/fixtures/search-storage.json"
    ))
    .unwrap()
}
pub async fn seed(db: &Database) {
    for record in fixture()["records"].as_array().unwrap() {
        db.upsert_prepared_search_document(
            serde_json::from_value(record["stored"].clone()).unwrap(),
        )
        .await
        .unwrap();
    }
}
pub fn assert_json(actual: Value, expected: Value) {
    match (&actual, &expected) {
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64()),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                assert_json(a.clone(), b.clone());
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "different fields: {actual} != {expected}");
            for (k, v) in a {
                assert_json(
                    v.clone(),
                    b.get(k).unwrap_or_else(|| panic!("missing {k}")).clone(),
                );
            }
        }
        _ => assert_eq!(actual, expected),
    }
}
