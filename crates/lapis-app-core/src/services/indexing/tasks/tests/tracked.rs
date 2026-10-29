use super::support::*;
use crate::services::indexing::owner::Clock;
use crate::services::{TaskLifecyclePolicy, TrackedTaskProvider, daily::*};
use lapis_task_services::scan_task_occurrence_observations;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;

pub(super) fn policy() -> TaskLifecyclePolicy {
    TaskLifecyclePolicy {
        day_parts: serde_json::from_value(json!({
            "afternoonStartsAt":"12:00", "eveningStartsAt":"18:00"
        }))
        .unwrap(),
        daily: DailyDocumentSettings::default(),
        format: DailyFormatContext {
            locale: "en-US".into(),
            first_day: 7,
            minimal_days: 4,
        },
    }
}
pub(super) fn corpus() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/task-tracked-today.json"
    )))
    .unwrap()
}
pub(super) fn raw() -> String {
    corpus()["cases"][0]["files"]["Habit.md"]
        .as_str()
        .unwrap()
        .into()
}
fn visible(root: &Path, path: &Path, result: &mut BTreeMap<String, String>) {
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            visible(root, &entry.path(), result);
        } else {
            result.insert(
                entry
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .into(),
                fs::read_to_string(entry.path()).unwrap(),
            );
        }
    }
}

#[tokio::test]
async fn tracked_today_replays_frozen_retained_plugin_snapshots_on_existing_owner() {
    let corpus = corpus();
    assert_eq!(corpus["cases"].as_array().unwrap().len(), 15);
    let mut attempts = 0;
    for case in corpus["cases"].as_array().unwrap() {
        let profile = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        for field in ["files", "daily"] {
            for (path, raw) in case[field].as_object().unwrap() {
                write(&root, path, raw.as_str().unwrap());
            }
        }
        let runtime = runtime(&profile);
        let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
        let date = Arc::new(Mutex::new("2026-08-20".to_string()));
        let current = date.clone();
        vault
            .start_indexing(
                false,
                true,
                Clock {
                    today: Arc::new(move || current.lock().unwrap().clone()),
                    interval: Duration::from_millis(25),
                },
            )
            .await
            .unwrap();
        until(async || vault.indexing_status().unwrap().passes > 0).await;
        assert!(vault.indexing_status().unwrap().tracked.is_none());
        let original_owner = vault
            .indexing
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .status
            .clone();
        for expected in case["expected"]["attempts"].as_array().unwrap() {
            let day = expected["date"].as_str().unwrap();
            *date.lock().unwrap() = day.into();
            let before = vault.indexing_status().unwrap().passes;
            vault
                .set_tracked_task_materialization(Some(TrackedTaskProvider::Default(policy())))
                .await
                .unwrap();
            until(async || {
                let status = vault.indexing_status().unwrap();
                status.passes > before && status.tracked.as_ref().is_some_and(|row| row.date == day)
            })
            .await;
            assert!(Arc::ptr_eq(
                &original_owner,
                &vault.indexing.lock().unwrap().as_ref().unwrap().status
            ));
            let status = vault.indexing_status().unwrap();
            assert!(status.error.is_none(), "{}: {:?}", case["name"], status);
            assert!(status.tracked.unwrap().failures.is_empty());
            let mut actual = BTreeMap::new();
            visible(root.path(), root.path(), &mut actual);
            let mut files = expected["tasks"].as_object().unwrap().clone();
            files.extend(expected["daily"].as_object().unwrap().clone());
            assert_eq!(
                serde_json::to_value(&actual).unwrap(),
                json!(files),
                "{} / {day}",
                case["name"]
            );
            for (path, observations) in expected["observations"].as_object().unwrap() {
                assert_eq!(
                    serde_json::to_value(scan_task_occurrence_observations(&actual[path], path))
                        .unwrap(),
                    *observations,
                    "{} / {day}",
                    case["name"]
                );
            }
            attempts += 1;
        }
        assert_eq!(vault.status().unwrap().clients, 0);
        close(runtime).await;
    }
    assert_eq!(attempts, 45);
}
