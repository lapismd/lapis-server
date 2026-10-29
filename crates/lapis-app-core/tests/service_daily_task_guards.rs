use lapis_app_core::services::{
    Runtime, TaskWriteError, VaultService, Work,
    daily::{
        DailyDocumentOperationError, DailyDocumentSettings, DailyFormatContext,
        DefaultTaskObservationError,
    },
};
use lapis_app_protocol::{ErrorCode, ExpectedVersion, VaultPhase};
use lapis_task_services::{TaskOccurrenceCompletion, TaskOccurrenceMaterialization};
use serde_json::Value;
use std::{
    fs,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

fn context() -> DailyFormatContext {
    DailyFormatContext {
        locale: "en-US".into(),
        first_day: 7,
        minimal_days: 4,
    }
}
fn source() -> (
    Value,
    TaskOccurrenceMaterialization,
    TaskOccurrenceCompletion,
) {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/task-lifecycle.json")).unwrap();
    let case = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "complete / schedule / count")
        .unwrap()
        .clone();
    let observation = &case["expected"]["attempts"][0]["planner"]["value"]["observation"];
    let input = serde_json::from_value(observation["input"].clone()).unwrap();
    let completion = serde_json::from_value(observation["completion"].clone()).unwrap();
    (case, input, completion)
}
fn seeded() -> (tempfile::TempDir, Arc<VaultService>, Value) {
    let root = tempfile::tempdir().unwrap();
    let (case, _, _) = source();
    for (path, raw) in case["daily"].as_object().unwrap() {
        fs::create_dir_all(root.path().join(path).parent().unwrap()).unwrap();
        fs::write(root.path().join(path), raw.as_str().unwrap()).unwrap();
    }
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    (root, vault, case)
}
fn process(
    work: &Work,
) -> Result<lapis_app_core::services::TaskObservationWrite, DefaultTaskObservationError> {
    let (_, input, completion) = source();
    work.process_default_daily_task_occurrence(
        &DailyDocumentSettings::default(),
        &context(),
        &input,
        Some(&completion),
    )
}
fn service(error: DefaultTaskObservationError) -> ErrorCode {
    match error {
        DefaultTaskObservationError::Write(TaskWriteError::Service(error)) => error.code(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn completed_same_date_retries_preserve_mtime_version_and_observation_identity() {
    let (root, vault, _) = seeded();
    let work = vault.begin_work().unwrap();
    let first = process(&work).unwrap();
    let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1_500_000_000);
    fs::File::options()
        .write(true)
        .open(root.path().join(&first.document.path))
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(stamp))
        .unwrap();
    for _ in 0..2 {
        let repeated = process(&work).unwrap();
        assert_eq!(repeated.document.text, first.document.text);
        assert_eq!(repeated.document.version, first.document.version);
        assert_eq!(
            serde_json::to_value(repeated.observation).unwrap(),
            serde_json::to_value(&first.observation).unwrap()
        );
        assert_eq!(
            fs::metadata(root.path().join(&first.document.path))
                .unwrap()
                .modified()
                .unwrap(),
            stamp
        );
    }
}

#[test]
fn dirty_attached_detached_and_noop_daily_writes_keep_original_bytes() {
    for detached in [false, true] {
        for noop in [false, true] {
            let (root, vault, case) = seeded();
            let work = vault.begin_work().unwrap();
            if noop {
                process(&work).unwrap();
            }
            let path = case["daily"].as_object().unwrap().keys().next().unwrap();
            let before = fs::read_to_string(root.path().join(path)).unwrap();
            let mut client = Some(vault.attach().unwrap());
            let editor = client.as_ref().unwrap().open_editor(path).unwrap();
            client.as_ref().unwrap().mark_dirty(editor, 1).unwrap();
            if detached {
                drop(client.take());
            }
            assert_eq!(service(process(&work).unwrap_err()), ErrorCode::Busy);
            assert_eq!(fs::read_to_string(root.path().join(path)).unwrap(), before);
            if detached {
                let recovered = vault.attach().unwrap();
                recovered.recover_editor(editor).unwrap();
                recovered.discard_editor(editor).unwrap();
            } else {
                client.as_ref().unwrap().discard_editor(editor).unwrap();
            }
        }
    }
}

#[test]
fn dirty_hardlink_aliases_fence_default_processing() {
    let (root, vault, case) = seeded();
    let path = case["daily"].as_object().unwrap().keys().next().unwrap();
    fs::hard_link(root.path().join(path), root.path().join("Alias.txt")).unwrap();
    let client = vault.attach().unwrap();
    let editor = client.open_editor("Alias.txt").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    let work = vault.begin_work().unwrap();
    let before = fs::read_to_string(root.path().join(path)).unwrap();
    assert_eq!(service(process(&work).unwrap_err()), ErrorCode::Busy);
    assert_eq!(fs::read_to_string(root.path().join(path)).unwrap(), before);
    client.discard_editor(editor).unwrap();
}

#[test]
fn existing_daily_noops_still_require_the_current_content_version() {
    let (_root, vault, case) = seeded();
    let work = vault.begin_work().unwrap();
    let path = case["daily"].as_object().unwrap().keys().next().unwrap();
    let original = work.read_text(path).unwrap();
    let first = process(&work).unwrap();
    let (_, input, completion) = source();
    let error = work
        .process_task_occurrence(
            path,
            &ExpectedVersion::Version {
                version: original.version,
            },
            &input,
            Some(&completion),
        )
        .unwrap_err();
    let TaskWriteError::Service(error) = error else {
        panic!("lost native error")
    };
    assert_eq!(error.code(), ErrorCode::Conflict);
    assert_eq!(work.read_text(path).unwrap().text, first.document.text);
}

#[test]
fn rejected_completion_keeps_the_ensured_seed_without_a_partial_observation() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let work = vault.begin_work().unwrap();
    let (_, input, _) = source();
    let seed = "---\ntype: daily-note\ndate: 2026-08-20\n---\n\n# 2026-08-20\n";
    let invalid = TaskOccurrenceCompletion::Count {
        value: -1.0,
        unit: "pages".into(),
    };
    let error = work
        .process_default_daily_task_occurrence(
            &DailyDocumentSettings::default(),
            &context(),
            &input,
            Some(&invalid),
        )
        .unwrap_err();
    let DefaultTaskObservationError::Write(TaskWriteError::Transform(error)) = error else {
        panic!("lost transform error")
    };
    assert_eq!(error.code.as_deref(), Some("invalid-task-occurrence-value"));
    assert_eq!(
        fs::read_to_string(root.path().join("daily/2026-08-20.md")).unwrap(),
        seed
    );
}

#[test]
fn provider_failure_remains_distinct_and_preserves_the_occupied_target() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("daily")).unwrap();
    fs::write(root.path().join("daily/2026-08-20.md"), "Unrelated").unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let work = vault.begin_work().unwrap();
    let error = process(&work).unwrap_err();
    assert!(matches!(
        error,
        DefaultTaskObservationError::Provider(DailyDocumentOperationError::Policy(_))
    ));
    assert_eq!(
        fs::read_to_string(root.path().join("daily/2026-08-20.md")).unwrap(),
        "Unrelated"
    );
}

#[test]
fn accepted_default_processing_finishes_after_detach_while_close_drains() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    let work = vault.begin_work().unwrap();
    drop(client);
    let (closed_tx, closed_rx) = mpsc::channel();
    thread::scope(|scope| {
        let owner = vault.clone();
        scope.spawn(move || {
            owner.close().unwrap();
            closed_tx.send(()).unwrap();
        });
        while vault.status().unwrap().phase == VaultPhase::Open {
            thread::yield_now();
        }
        assert_eq!(vault.begin_work().err().unwrap().code(), ErrorCode::Closed);
        let value = process(&work).unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join(value.document.path)).unwrap(),
            value.document.text
        );
        assert!(closed_rx.recv_timeout(Duration::from_millis(20)).is_err());
        drop(work);
        closed_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    });
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
}
