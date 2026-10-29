use super::*;
use crate::services::{Runtime, TaskWriteError};
use lapis_task_services::TaskTransformError;
use serde_json::{Value, json};
use std::{fs, path::Path};

fn put(root: &Path, path: &str, text: &str) {
    let target = root.join(path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, text).unwrap();
}
fn numbers(value: Value) -> Value {
    match value {
        Value::Number(value) => json!(value.as_f64().unwrap()),
        Value::Array(values) => Value::Array(values.into_iter().map(numbers).collect()),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, numbers(value)))
                .collect(),
        ),
        other => other,
    }
}
fn transform(error: TaskRecoveryError) -> TaskTransformError {
    match error {
        TaskRecoveryError::Task(TaskWriteError::Transform(error)) => error,
        TaskRecoveryError::Partial { cause, .. } => transform(*cause),
        other => panic!("lost retained source error: {other:?}"),
    }
}
fn replay(source: &str, counts: [usize; 2]) {
    let fixture: Value = serde_json::from_str(source).unwrap();
    let mut outcomes = [0, 0];
    for case in fixture["cases"].as_array().unwrap() {
        if case.get("failDailyWrites").is_some()
            || case.get("failTaskWrites").is_some()
            || [
                "noncanonical daily date",
                "daily provider creates explicit fixture seed",
            ]
            .contains(&case["name"].as_str().unwrap())
        {
            continue;
        }
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let root = base.join("vault");
        let profile = base.join("profile");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&profile).unwrap();
        let path = case["path"].as_str().unwrap();
        put(&root, path, case["raw"].as_str().unwrap());
        for (path, text) in case["daily"].as_object().unwrap() {
            put(&root, path, text.as_str().unwrap());
        }
        let vault = Runtime::default().admit(Uuid::new_v4(), &root).unwrap();
        let mut journal = TaskLifecycleJournal::open(&profile, &vault).unwrap();
        let policy = TaskLifecyclePolicy {
            day_parts: serde_json::from_value(case["settings"].clone()).unwrap(),
            daily: DailyDocumentSettings::default(),
            format: DailyFormatContext {
                locale: "en-US".into(),
                first_day: 7,
                minimal_days: 4,
            },
        };
        for (action, expected) in case["actions"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"]["attempts"].as_array().unwrap())
        {
            let work = vault.begin_work().unwrap();
            let before = work.read_text(path).unwrap();
            let request = TaskLifecycleRequest {
                request_id: Uuid::new_v4(),
                path: path.into(),
                expected_version: before.version,
                action: serde_json::from_value(action.clone()).unwrap(),
            };
            let result = journal.submit(work, request, &policy);
            if let Some(expected) = expected["planner"].get("error") {
                assert_eq!(
                    numbers(serde_json::to_value(transform(result.unwrap_err())).unwrap()),
                    numbers(expected.clone()),
                    "{}",
                    case["name"]
                );
                outcomes[1] += 1;
            } else {
                if let Some(expected) = expected["result"].get("error") {
                    assert_eq!(
                        numbers(serde_json::to_value(transform(result.unwrap_err())).unwrap()),
                        numbers(expected.clone()),
                        "{}",
                        case["name"]
                    );
                } else {
                    let result = result.unwrap();
                    assert_eq!(result.disposition, TaskLifecycleDisposition::Completed);
                    assert_eq!(
                        result.task_replacement, expected["taskRaw"],
                        "{}",
                        case["name"]
                    );
                }
                outcomes[0] += 1;
            }
            assert_eq!(
                fs::read_to_string(root.join(path)).unwrap(),
                expected["taskRaw"],
                "{}",
                case["name"]
            );
            for (path, text) in expected["daily"].as_object().unwrap() {
                assert_eq!(
                    fs::read_to_string(root.join(path)).unwrap(),
                    text.as_str().unwrap(),
                    "{}",
                    case["name"]
                );
            }
        }
    }
    assert_eq!(outcomes, counts);
}
#[test]
fn durable_default_lifecycle_matches_all_retained_repository_attempts() {
    replay(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/task-lifecycle.json"
        )),
        [58, 34],
    );
}
#[test]
fn durable_default_lifecycle_matches_all_retained_duration_attempts() {
    replay(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/task-lifecycle-duration.json"
        )),
        [11, 7],
    );
}
