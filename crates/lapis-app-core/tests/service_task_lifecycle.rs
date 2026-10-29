use lapis_app_core::services::{
    Runtime, TaskLifecycleError, TaskWriteError,
    daily::{DailyDocumentSettings, DailyFormatContext, DefaultTaskObservationError},
};
use lapis_app_protocol::{ErrorCode, ExpectedVersion};
use lapis_task_services::{TaskLifecycleAction, parse_task_document};
use serde_json::{Value, json};
use std::{fs, path::Path};
use uuid::Uuid;

fn context() -> DailyFormatContext {
    DailyFormatContext {
        locale: "en-US".into(),
        first_day: 7,
        minimal_days: 4,
    }
}
fn put(root: &Path, path: &str, raw: &str) {
    let target = root.join(path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, raw).unwrap();
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

#[test]
fn ordered_default_lifecycle_matches_retained_repository_attempts() {
    replay(include_str!("fixtures/task-lifecycle.json"), [58, 34]);
}

#[test]
fn ordered_default_lifecycle_preserves_valid_duration_repository_attempts() {
    replay(
        include_str!("fixtures/task-lifecycle-duration.json"),
        [11, 7],
    );
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
        let root = tempfile::tempdir().unwrap();
        let path = case["path"].as_str().unwrap();
        put(root.path(), path, case["raw"].as_str().unwrap());
        for (path, raw) in case["daily"].as_object().unwrap() {
            put(root.path(), path, raw.as_str().unwrap());
        }
        let runtime = Runtime::default();
        let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
        for (action, expected) in case["actions"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"]["attempts"].as_array().unwrap())
        {
            let work = vault.begin_work().unwrap();
            let before = work.read_text(path).unwrap();
            let action: TaskLifecycleAction = serde_json::from_value(action.clone()).unwrap();
            let settings = serde_json::from_value(case["settings"].clone()).unwrap();
            let result = work.prepare_task_lifecycle(
                path,
                &ExpectedVersion::Version {
                    version: before.version,
                },
                action,
                &settings,
            );
            if let Some(error) = expected["planner"].get("error") {
                let TaskWriteError::Transform(actual) = result.err().unwrap() else {
                    panic!("lost transform error")
                };
                assert_eq!(
                    numbers(serde_json::to_value(actual).unwrap()),
                    numbers(error.clone()),
                    "{}",
                    case["name"]
                );
                outcomes[1] += 1;
            } else {
                let result = result
                    .unwrap()
                    .persist_default_daily(&DailyDocumentSettings::default(), &context());
                if expected["result"].get("error").is_some() {
                    let TaskLifecycleError::Daily(DefaultTaskObservationError::Write(
                        TaskWriteError::Transform(actual),
                    )) = result.unwrap_err()
                    else {
                        panic!("lost daily transform error for {}", case["name"])
                    };
                    assert_eq!(
                        numbers(serde_json::to_value(actual).unwrap()),
                        numbers(expected["result"]["error"].clone()),
                        "{}",
                        case["name"]
                    );
                } else {
                    let result = result.unwrap();
                    assert_eq!(
                        result.task.text,
                        expected["taskRaw"].as_str().unwrap(),
                        "{}",
                        case["name"]
                    );
                    assert_eq!(
                        numbers(
                            serde_json::to_value(parse_task_document(&result.task.text, path))
                                .unwrap()
                        ),
                        numbers(expected["task"].clone()),
                        "{}",
                        case["name"]
                    );
                    if let Some(daily) = result.daily {
                        let observations = &expected["observations"][&daily.document.path];
                        assert!(observations.as_array().unwrap().iter().any(|value| numbers(
                            value.clone()
                        ) == numbers(
                            serde_json::to_value(&daily.observation).unwrap()
                        )));
                    }
                }
                outcomes[0] += 1;
            }
            assert_eq!(
                fs::read_to_string(root.path().join(path)).unwrap(),
                expected["taskRaw"].as_str().unwrap(),
                "{}",
                case["name"]
            );
            for (path, raw) in expected["daily"].as_object().unwrap() {
                assert_eq!(
                    fs::read_to_string(root.path().join(path)).unwrap(),
                    raw.as_str().unwrap(),
                    "{}",
                    case["name"]
                );
            }
        }
    }
    assert_eq!(outcomes, counts);
}

#[test]
fn original_task_freshness_is_checked_before_any_daily_creation() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/task-lifecycle.json")).unwrap();
    let case = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "complete / schedule / count")
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = case["path"].as_str().unwrap();
    put(root.path(), path, case["raw"].as_str().unwrap());
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let work = vault.begin_work().unwrap();
    let before = work.read_text(path).unwrap();
    let prepared = work
        .prepare_task_lifecycle(
            path,
            &ExpectedVersion::Version {
                version: before.version,
            },
            serde_json::from_value(case["actions"][0].clone()).unwrap(),
            &serde_json::from_value(case["settings"].clone()).unwrap(),
        )
        .unwrap();
    fs::write(root.path().join(path), "external change").unwrap();
    let error = prepared
        .persist_default_daily(&DailyDocumentSettings::default(), &context())
        .unwrap_err();
    let TaskLifecycleError::Admission(TaskWriteError::Service(error)) = error else {
        panic!("lost admission error")
    };
    assert_eq!(error.code(), ErrorCode::Conflict);
    assert!(!root.path().join("daily").exists());
    assert_eq!(
        fs::read_to_string(root.path().join(path)).unwrap(),
        "external change"
    );
}
