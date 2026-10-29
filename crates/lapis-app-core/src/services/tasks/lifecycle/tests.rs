use super::*;
use crate::services::{Runtime, TaskWriteError, VaultService};
use lapis_app_protocol::ErrorCode;
use serde_json::Value;
use std::{
    fs,
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};
use uuid::Uuid;

fn setup() -> (tempfile::TempDir, Arc<VaultService>, Value) {
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/task-lifecycle.json"
    )))
    .unwrap();
    let case = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "complete / schedule / count")
        .unwrap()
        .clone();
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("tasks")).unwrap();
    fs::write(
        root.path().join("tasks/Read.md"),
        case["raw"].as_str().unwrap(),
    )
    .unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    (root, vault, case)
}
fn prepare(vault: &Arc<VaultService>, case: &Value) -> PreparedTaskLifecycle {
    let work = vault.begin_work().unwrap();
    let expected = ExpectedVersion::Version {
        version: work.read_text("tasks/Read.md").unwrap().version,
    };
    work.prepare_task_lifecycle(
        "tasks/Read.md",
        &expected,
        serde_json::from_value(case["actions"][0].clone()).unwrap(),
        &serde_json::from_value(case["settings"].clone()).unwrap(),
    )
    .unwrap()
}
fn context() -> DailyFormatContext {
    DailyFormatContext {
        locale: "en-US".into(),
        first_day: 7,
        minimal_days: 4,
    }
}

#[test]
fn later_original_conflict_preserves_pinned_partial_and_daily_before_task_order() {
    let (root, vault, case) = setup();
    let original_version = vault
        .begin_work()
        .unwrap()
        .read_text("tasks/Read.md")
        .unwrap()
        .version;
    let error = persist(
        prepare(&vault, &case),
        &DailyDocumentSettings::default(),
        &context(),
        || {
            assert_eq!(
                fs::read_to_string(root.path().join("tasks/Read.md")).unwrap(),
                case["raw"]
            );
            assert!(
                fs::read_to_string(root.path().join("daily/2026-08-20.md"))
                    .unwrap()
                    .contains("task-occurrence")
            );
            fs::write(root.path().join("tasks/Read.md"), "New handwritten Task").unwrap();
        },
    )
    .unwrap_err();
    let TaskLifecycleError::Partial(partial) = error else {
        panic!("lost partial result")
    };
    assert_eq!(partial.original.version, original_version);
    assert_eq!(partial.original.text, case["raw"]);
    assert_eq!(
        partial.task_replacement,
        case["expected"]["attempts"][0]["taskRaw"]
    );
    assert_eq!(
        serde_json::to_value(partial.action).unwrap(),
        serde_json::to_value(
            serde_json::from_value::<TaskLifecycleAction>(case["actions"][0].clone()).unwrap()
        )
        .unwrap()
    );
    assert_eq!(
        partial.daily.observation.record.occurrence_date,
        "2026-08-20"
    );
    assert_eq!(
        partial.daily.document.text,
        fs::read_to_string(root.path().join(&partial.daily.document.path)).unwrap()
    );
    assert_eq!(
        partial.daily.document.version,
        vault
            .begin_work()
            .unwrap()
            .read_text(&partial.daily.document.path)
            .unwrap()
            .version
    );
    let TaskWriteError::Service(error) = partial.cause else {
        panic!("lost conflict cause")
    };
    assert_eq!(error.code(), ErrorCode::Conflict);
    assert_eq!(
        fs::read_to_string(root.path().join("tasks/Read.md")).unwrap(),
        "New handwritten Task"
    );
}

#[test]
fn later_dirty_claims_on_original_or_hardlink_alias_retain_explicit_partial() {
    for alias in [false, true] {
        let (root, vault, case) = setup();
        let path = if alias {
            "tasks/Alias.md"
        } else {
            "tasks/Read.md"
        };
        if alias {
            fs::hard_link(root.path().join("tasks/Read.md"), root.path().join(path)).unwrap();
        }
        let client = vault.attach().unwrap();
        let editor = client.open_editor(path).unwrap();
        let error = persist(
            prepare(&vault, &case),
            &DailyDocumentSettings::default(),
            &context(),
            || {
                client.mark_dirty(editor, 1).unwrap();
            },
        )
        .unwrap_err();
        let TaskLifecycleError::Partial(partial) = error else {
            panic!("lost partial")
        };
        let TaskWriteError::Service(error) = partial.cause else {
            panic!("lost writer error")
        };
        assert_eq!(error.code(), ErrorCode::Busy);
        assert_eq!(
            fs::read_to_string(root.path().join("tasks/Read.md")).unwrap(),
            case["raw"]
        );
        client.discard_editor(editor).unwrap();
    }
}

#[test]
fn concurrent_prepared_operations_cannot_publish_a_second_daily_after_advancement() {
    let (root, vault, case) = setup();
    let first = prepare(&vault, &case);
    let second = prepare(&vault, &case);
    let (daily_tx, daily_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    thread::scope(|scope| {
        let first = scope.spawn(move || {
            persist(first, &DailyDocumentSettings::default(), &context(), || {
                daily_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            })
        });
        daily_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let before = fs::read_to_string(root.path().join("daily/2026-08-20.md")).unwrap();
        scope.spawn(move || {
            result_tx
                .send(second.persist_default_daily(&DailyDocumentSettings::default(), &context()))
                .unwrap();
        });
        assert!(result_rx.recv_timeout(Duration::from_millis(30)).is_err());
        release_tx.send(()).unwrap();
        first.join().unwrap().unwrap();
        let TaskLifecycleError::Admission(TaskWriteError::Service(error)) = result_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap_err()
        else {
            panic!("lost original guard")
        };
        assert_eq!(error.code(), ErrorCode::Conflict);
        assert_eq!(
            fs::read_to_string(root.path().join("daily/2026-08-20.md")).unwrap(),
            before
        );
    });
    assert_eq!(
        fs::read_to_string(root.path().join("tasks/Read.md")).unwrap(),
        case["expected"]["attempts"][0]["taskRaw"]
    );
}

#[test]
fn daily_dirty_failure_never_attempts_original_advancement() {
    let (root, vault, case) = setup();
    fs::create_dir(root.path().join("daily")).unwrap();
    fs::write(
        root.path().join("daily/2026-08-20.md"),
        case["daily"]["daily/2026-08-20.md"].as_str().unwrap(),
    )
    .unwrap();
    let client = vault.attach().unwrap();
    let editor = client.open_editor("daily/2026-08-20.md").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    let error = persist(
        prepare(&vault, &case),
        &DailyDocumentSettings::default(),
        &context(),
        || panic!("daily failure reached original write"),
    )
    .unwrap_err();
    assert!(matches!(error, TaskLifecycleError::Daily(_)));
    assert_eq!(
        fs::read_to_string(root.path().join("tasks/Read.md")).unwrap(),
        case["raw"]
    );
    assert_eq!(
        fs::read_to_string(root.path().join("daily/2026-08-20.md")).unwrap(),
        case["daily"]["daily/2026-08-20.md"]
    );
    client.discard_editor(editor).unwrap();
}
