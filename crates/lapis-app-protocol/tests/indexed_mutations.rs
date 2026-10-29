use lapis_app_protocol::database::indexed::mutations::IndexedFile;
use serde_json::{Value, json};
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/indexed-mutations.json"
    ))
    .unwrap()
}
fn normalized(value: Value) -> Value {
    match value {
        Value::Number(value) => json!(value.as_f64().unwrap()),
        Value::Array(values) => Value::Array(values.into_iter().map(normalized).collect()),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, normalized(value)))
                .collect(),
        ),
        value => value,
    }
}
#[test]
fn indexed_upsert_distinguishes_omitted_null_and_complete_task_records() {
    for index in [0, 1, 2] {
        let value = fixture()["cases"][index]["steps"][0]["record"].clone();
        let record: IndexedFile = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(record.task.is_some(), index != 0);
        if index == 1 {
            assert!(record.task.as_ref().unwrap().is_some());
        }
        if index == 2 {
            assert!(record.task.as_ref().unwrap().is_none());
        }
        let output = serde_json::to_value(record).unwrap();
        assert_eq!(output.get("task").is_some(), value.get("task").is_some());
        assert_eq!(normalized(output), normalized(value));
    }
}
#[test]
fn indexed_upsert_rejects_sql_native_authority_and_unknown_nested_task_fields() {
    let base = fixture()["cases"][1]["steps"][0]["record"].clone();
    for key in ["sql", "databasePath", "vaultRoot"] {
        let mut bad = base.clone();
        bad[key] = json!("forbidden");
        assert!(serde_json::from_value::<IndexedFile>(bad).is_err());
    }
    let mut bad = base.clone();
    bad["task"]["executable"] = json!("forbidden");
    assert!(serde_json::from_value::<IndexedFile>(bad).is_err());
    let mut bad = base;
    bad["file"]["mtime"] = json!("invalid");
    assert!(serde_json::from_value::<IndexedFile>(bad).is_err());
}
