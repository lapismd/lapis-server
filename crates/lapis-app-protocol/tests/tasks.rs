use lapis_app_protocol::database::tasks::{TaskLookup, TaskQuery, record::TaskRecord};
use serde_json::{Value, json};

#[test]
fn task_contract_retains_complete_v4_json_and_distinguishes_optional_missing_and_null() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/task-projection.json"
    ))
    .unwrap();
    for record in fixture["records"].as_array().unwrap() {
        let typed: TaskRecord = serde_json::from_value(record.clone()).unwrap();
        assert_eq!(serde_json::to_value(typed).unwrap(), *record);
    }
    let mut nullable = fixture["records"][0].clone();
    nullable["repeat"] = Value::Null;
    nullable["tracking"] = Value::Null;
    nullable["structure"] = Value::Null;
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<TaskRecord>(nullable.clone()).unwrap())
            .unwrap(),
        nullable
    );
    let mut invalid = fixture["records"][0].clone();
    invalid["sql"] = json!("UPDATE files SET hash='bad'");
    assert!(serde_json::from_value::<TaskRecord>(invalid).is_err());
    for bad in [
        json!({"view":"archive"}),
        json!({"sql":"SELECT * FROM files"}),
        json!({"kind":"note"}),
    ] {
        assert!(serde_json::from_value::<TaskQuery>(bad).is_err());
    }
    assert!(
        serde_json::from_value::<TaskLookup>(json!({"path":"task.md","root":"/tmp/other"}))
            .is_err()
    );
}

#[test]
fn task_tracking_variants_and_occurrence_states_retain_the_source_catalogue() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/task-projection.json"
    ))
    .unwrap();
    for tracking in [
        json!({"type":"boolean"}),
        json!({"type":"count","unit":"pages","target":20.0}),
        json!({"type":"duration","target":"45m"}),
        json!({"type":"number","unit":"km","target":3.5}),
    ] {
        let mut row = fixture["records"][0].clone();
        row["tracking"] = tracking.clone();
        let parsed: TaskRecord = serde_json::from_value(row).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap()["tracking"], tracking);
    }
    for state in [
        "current",
        "overdue",
        "future",
        "exhausted",
        "paused",
        "unsupported",
    ] {
        let mut row = fixture["records"][0].clone();
        row["effectiveOccurrenceState"] = json!(state);
        assert!(serde_json::from_value::<TaskRecord>(row).is_ok());
    }
}
