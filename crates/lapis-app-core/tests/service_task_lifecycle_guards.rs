use lapis_app_core::services::{
    Runtime, TaskLifecycleError, TaskWriteError, VaultService,
    daily::{DailyDocumentSettings, DailyFormatContext},
};
use lapis_app_protocol::{ErrorCode, ExpectedVersion, VaultPhase};
use lapis_task_services::TaskLifecycleAction;
use serde_json::Value;
use std::{
    fs,
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};
use uuid::Uuid;

fn seeded() -> (tempfile::TempDir, Arc<VaultService>, Value) {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/task-lifecycle.json")).unwrap();
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
fn context() -> DailyFormatContext {
    DailyFormatContext {
        locale: "en-US".into(),
        first_day: 7,
        minimal_days: 4,
    }
}
fn prepared(
    vault: &Arc<VaultService>,
    case: &Value,
) -> lapis_app_core::services::PreparedTaskLifecycle {
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

#[test]
fn dirty_original_admission_prevents_daily_creation_for_attached_and_detached_editors() {
    for detached in [false, true] {
        let (root, vault, case) = seeded();
        let operation = prepared(&vault, &case);
        let mut client = Some(vault.attach().unwrap());
        let editor = client
            .as_ref()
            .unwrap()
            .open_editor("tasks/Read.md")
            .unwrap();
        client.as_ref().unwrap().mark_dirty(editor, 1).unwrap();
        if detached {
            drop(client.take());
        }
        let error = operation
            .persist_default_daily(&DailyDocumentSettings::default(), &context())
            .unwrap_err();
        let TaskLifecycleError::Admission(TaskWriteError::Service(error)) = error else {
            panic!("lost admission error")
        };
        assert_eq!(error.code(), ErrorCode::Busy);
        assert!(!root.path().join("daily").exists());
        assert_eq!(
            fs::read_to_string(root.path().join("tasks/Read.md")).unwrap(),
            case["raw"]
        );
        if detached {
            let recovered = vault.attach().unwrap();
            recovered.recover_editor(editor).unwrap();
            recovered.discard_editor(editor).unwrap();
        } else {
            client.unwrap().discard_editor(editor).unwrap();
        }
    }
}

#[test]
fn provider_failure_keeps_original_task_and_unrelated_daily_target() {
    let (root, vault, case) = seeded();
    fs::create_dir(root.path().join("daily")).unwrap();
    fs::write(root.path().join("daily/2026-08-20.md"), "Handwritten note").unwrap();
    let error = prepared(&vault, &case)
        .persist_default_daily(&DailyDocumentSettings::default(), &context())
        .unwrap_err();
    assert!(matches!(error, TaskLifecycleError::Daily(_)));
    assert_eq!(
        fs::read_to_string(root.path().join("tasks/Read.md")).unwrap(),
        case["raw"]
    );
    assert_eq!(
        fs::read_to_string(root.path().join("daily/2026-08-20.md")).unwrap(),
        "Handwritten note"
    );
}

#[test]
fn prepared_operation_owns_work_through_detach_and_close_drain() {
    let (root, vault, case) = seeded();
    let client = vault.attach().unwrap();
    let operation = prepared(&vault, &case);
    drop(client);
    let (tx, rx) = mpsc::channel();
    thread::scope(|scope| {
        let closing = vault.clone();
        scope.spawn(move || {
            closing.close().unwrap();
            tx.send(()).unwrap();
        });
        while vault.status().unwrap().phase == VaultPhase::Open {
            thread::yield_now();
        }
        assert_eq!(vault.begin_work().err().unwrap().code(), ErrorCode::Closed);
        assert!(rx.recv_timeout(Duration::from_millis(20)).is_err());
        let result = operation
            .persist_default_daily(&DailyDocumentSettings::default(), &context())
            .unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join("tasks/Read.md")).unwrap(),
            result.task.text
        );
        assert_eq!(result.task.text, case["expected"]["attempts"][0]["taskRaw"]);
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
    });
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
}

#[test]
fn invalid_source_plan_cannot_create_a_daily_seed() {
    let (root, vault, case) = seeded();
    let work = vault.begin_work().unwrap();
    let expected = ExpectedVersion::Version {
        version: work.read_text("tasks/Read.md").unwrap().version,
    };
    let action = TaskLifecycleAction::Complete {
        at: "2026-08-20T09:00:00Z".into(),
        local_date: "2026-08-20".into(),
        tracked_value: None,
    };
    assert!(matches!(
        work.prepare_task_lifecycle(
            "tasks/Read.md",
            &expected,
            action,
            &serde_json::from_value(case["settings"].clone()).unwrap()
        ),
        Err(TaskWriteError::Transform(_))
    ));
    assert!(!root.path().join("daily").exists());
    assert_eq!(
        fs::read_to_string(root.path().join("tasks/Read.md")).unwrap(),
        case["raw"]
    );
}
