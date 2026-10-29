use lapis_app_core::services::{
    Runtime, TaskWriteError,
    daily::{DailyDocumentSettings, DailyFormatContext, DefaultTaskObservationError},
};
use lapis_app_protocol::ExpectedVersion;
use lapis_task_services::{
    TaskOccurrenceCompletion, TaskOccurrenceMaterialization, scan_task_occurrence_observations,
};
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
fn transform(error: TaskWriteError) -> Value {
    match error {
        TaskWriteError::Transform(error) => serde_json::to_value(error).unwrap(),
        other => panic!("unexpected service error: {other:?}"),
    }
}

#[test]
fn daily_stage_replays_frozen_repository_observation_attempts_without_task_advancement() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/task-lifecycle.json")).unwrap();
    let mut outcomes = [0, 0, 0, 0];
    for case in fixture["cases"].as_array().unwrap() {
        let root = tempfile::tempdir().unwrap();
        let task_path = case["path"].as_str().unwrap();
        let task_raw = case["raw"].as_str().unwrap();
        put(root.path(), task_path, task_raw);
        for (path, raw) in case["daily"].as_object().unwrap() {
            put(root.path(), path, raw.as_str().unwrap());
        }
        let runtime = Runtime::default();
        let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
        let work = vault.begin_work().unwrap();
        for attempt in case["expected"]["attempts"].as_array().unwrap() {
            let Some(observation) = attempt["planner"]["value"].get("observation") else {
                continue;
            };
            let intents = attempt["intents"].as_array().unwrap();
            if intents
                .iter()
                .any(|intent| intent["kind"] == "daily-write" && intent["failed"] == true)
            {
                outcomes[3] += 1;
                continue;
            }
            let input: TaskOccurrenceMaterialization =
                serde_json::from_value(observation["input"].clone()).unwrap();
            let completion: Option<TaskOccurrenceCompletion> = observation
                .get("completion")
                .map(|value| serde_json::from_value(value.clone()).unwrap());
            let path = intents
                .iter()
                .find(|intent| intent["kind"] == "ensure-daily")
                .unwrap()["path"]
                .as_str()
                .unwrap();
            let result = if case["name"] == "noncanonical daily date" {
                let expected = ExpectedVersion::Version {
                    version: work.read_text(path).unwrap().version,
                };
                work.process_task_occurrence(path, &expected, &input, completion.as_ref())
                    .map_err(transform)
            } else {
                work.process_default_daily_task_occurrence(
                    &DailyDocumentSettings::default(),
                    &context(),
                    &input,
                    completion.as_ref(),
                )
                .map_err(|error| match error {
                    DefaultTaskObservationError::Write(error) => transform(error),
                    other => panic!("unexpected provider error: {other:?}"),
                })
            };
            outcomes[0] += 1;
            let write = intents
                .iter()
                .find(|intent| intent["kind"] == "daily-write");
            if let Some(write) = write {
                let result = result.unwrap_or_else(|error| panic!("{}: {error:?}", case["name"]));
                assert_eq!(result.document.path, path, "{}", case["name"]);
                assert_eq!(
                    result.document.text,
                    write["raw"].as_str().unwrap(),
                    "{}",
                    case["name"]
                );
                let expected: Vec<_> = attempt["observations"][path]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|value| {
                        value["taskId"] == input.task_id
                            && value["occurrenceDate"] == input.occurrence_date
                    })
                    .cloned()
                    .collect();
                assert_eq!(
                    numbers(serde_json::to_value(&result.observation).unwrap()),
                    numbers(expected[0].clone()),
                    "{}",
                    case["name"]
                );
                assert_eq!(expected.len(), 1);
                assert_eq!(
                    result.document.version,
                    work.read_text(path).unwrap().version
                );
                outcomes[1] += 1;
            } else {
                assert_eq!(
                    result.unwrap_err(),
                    attempt["result"]["error"],
                    "{}",
                    case["name"]
                );
                outcomes[2] += 1;
            }
            for (path, raw) in attempt["daily"].as_object().unwrap() {
                let current = fs::read_to_string(root.path().join(path)).unwrap();
                assert_eq!(current, raw.as_str().unwrap(), "{}", case["name"]);
                assert_eq!(
                    numbers(
                        serde_json::to_value(scan_task_occurrence_observations(&current, path))
                            .unwrap()
                    ),
                    numbers(attempt["observations"][path].clone()),
                    "{}",
                    case["name"]
                );
            }
            assert_eq!(
                fs::read_to_string(root.path().join(task_path)).unwrap(),
                task_raw
            );
        }
    }
    assert_eq!(outcomes, [38, 33, 5, 1]);
}

#[test]
fn combined_processing_preserves_all_tracking_kinds_and_crlf_source_ranges() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/daily-task-completions.json")).unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 8);
    for case in fixture["cases"].as_array().unwrap() {
        let root = tempfile::tempdir().unwrap();
        let path = case["path"].as_str().unwrap();
        let raw = case["raw"].as_str().unwrap();
        put(root.path(), path, raw);
        let original = scan_task_occurrence_observations(raw, path).remove(0);
        let input = TaskOccurrenceMaterialization {
            task_id: original.record.task_id,
            task_path: original.record.task_path,
            title: original.title,
            occurrence_date: original.record.occurrence_date,
            tracking: None,
            day_part: original.day_part,
        };
        let runtime = Runtime::default();
        let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
        let work = vault.begin_work().unwrap();
        let completion = serde_json::from_value(case["completion"].clone()).unwrap();
        let result = work
            .process_task_occurrence(
                path,
                &ExpectedVersion::Version {
                    version: work.read_text(path).unwrap().version,
                },
                &input,
                Some(&completion),
            )
            .unwrap();
        assert_eq!(
            result.document.text,
            case["expected"]["value"]["raw"].as_str().unwrap(),
            "{}",
            case["name"]
        );
        assert_eq!(
            numbers(serde_json::to_value(result.observation).unwrap()),
            numbers(case["expected"]["value"]["observations"][0].clone()),
            "{}",
            case["name"]
        );
        let repeated = work
            .process_task_occurrence(
                path,
                &ExpectedVersion::Version {
                    version: result.document.version.clone(),
                },
                &input,
                Some(&completion),
            )
            .unwrap();
        let actual = json!({
            "raw": result.document.text,
            "observations": scan_task_occurrence_observations(&result.document.text, path),
            "repeated": repeated.document.text,
            "repeatedObservations": scan_task_occurrence_observations(&repeated.document.text, path),
        });
        assert_eq!(
            numbers(actual),
            numbers(case["expected"]["value"].clone()),
            "{}",
            case["name"]
        );
        assert_eq!(work.read_text(path).unwrap().text, result.document.text);
    }
}
