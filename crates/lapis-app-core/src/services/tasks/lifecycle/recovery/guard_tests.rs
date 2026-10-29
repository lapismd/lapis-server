use super::*;
use crate::services::{Runtime, TaskWriteError};
use lapis_task_services::TaskLifecycleAction;
use serde_json::Value;
use std::{fs, sync::Arc};

fn fixture() -> (
    tempfile::TempDir,
    Arc<VaultService>,
    std::path::PathBuf,
    TaskLifecycleRequest,
    TaskLifecyclePolicy,
) {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let root = base.join("vault");
    let profile = base.join("profile");
    fs::create_dir_all(root.join("tasks")).unwrap();
    fs::create_dir(&profile).unwrap();
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
        .unwrap();
    fs::write(root.join("tasks/Read.md"), case["raw"].as_str().unwrap()).unwrap();
    let vault = Runtime::default().admit(Uuid::new_v4(), &root).unwrap();
    let work = vault.begin_work().unwrap();
    let request = TaskLifecycleRequest {
        request_id: Uuid::new_v4(),
        path: "tasks/Read.md".into(),
        expected_version: work.read_text("tasks/Read.md").unwrap().version,
        action: serde_json::from_value(case["actions"][0].clone()).unwrap(),
    };
    let policy = TaskLifecyclePolicy {
        day_parts: serde_json::from_value(case["settings"].clone()).unwrap(),
        daily: DailyDocumentSettings::default(),
        format: DailyFormatContext {
            locale: "en-US".into(),
            first_day: 7,
            minimal_days: 4,
        },
    };
    drop(work);
    (temp, vault, profile, request, policy)
}

#[test]
fn stale_or_dirty_original_fails_before_durable_acceptance_or_daily_creation() {
    for dirty in [false, true] {
        let (_temp, vault, profile, request, policy) = fixture();
        let root = vault.lock().unwrap().root.path().to_owned();
        let mut journal = TaskLifecycleJournal::open(&profile, &vault).unwrap();
        let client = vault.attach().unwrap();
        let editor = client.open_editor(&request.path).unwrap();
        if dirty {
            client.mark_dirty(editor, 1).unwrap();
        } else {
            fs::write(root.join(&request.path), "external change").unwrap();
        }
        assert!(
            journal
                .submit(vault.begin_work().unwrap(), request, &policy)
                .is_err()
        );
        assert!(journal.storage.ids().unwrap().is_empty());
        assert!(!root.join("daily").exists());
        client.discard_editor(editor).unwrap();
    }
}

#[test]
fn quantitative_failure_retains_an_ensured_seed_and_cannot_advance_on_cold_retry() {
    let (_temp, vault, profile, mut request, policy) = fixture();
    let id = vault.id;
    let root = vault.lock().unwrap().root.path().to_owned();
    let before = fs::read(root.join(&request.path)).unwrap();
    if let TaskLifecycleAction::Complete { tracked_value, .. } = &mut request.action {
        *tracked_value = Some(-1.0);
    }
    let mut journal = TaskLifecycleJournal::open(&profile, &vault).unwrap();
    let TaskRecoveryError::Partial { status, cause } = journal
        .submit(vault.begin_work().unwrap(), request.clone(), &policy)
        .unwrap_err()
    else {
        panic!("lost seed partial")
    };
    let TaskRecoveryError::Task(TaskWriteError::Transform(error)) = *cause else {
        panic!("lost quantitative error")
    };
    assert_eq!(error.message, "Tracked value must be zero or greater.");
    let daily = status.daily.unwrap();
    assert!(!daily.text.contains("task-occurrence"));
    assert_eq!(
        daily.text,
        fs::read_to_string(root.join(&daily.path)).unwrap()
    );
    drop(journal);
    vault.close().unwrap();
    drop(vault);
    let reopened = Runtime::default().admit(id, &root).unwrap();
    let mut journal = TaskLifecycleJournal::open(&profile, &reopened).unwrap();
    let cold = journal
        .inspect(&reopened.begin_work().unwrap(), request.request_id)
        .unwrap()
        .unwrap();
    let failure = cold.last_failure.unwrap();
    assert_eq!(failure.code, "invalid-task-occurrence-value");
    assert_eq!(failure.message, "Tracked value must be zero or greater.");
    assert!(
        journal
            .retry(reopened.begin_work().unwrap(), request.request_id)
            .is_err()
    );
    assert_eq!(fs::read(root.join(&request.path)).unwrap(), before);
    assert_eq!(
        fs::read_to_string(root.join(&daily.path)).unwrap(),
        daily.text
    );
}

#[test]
fn later_dirty_original_or_alias_returns_readable_partial_then_retries_once() {
    for alias in [false, true] {
        let (_temp, vault, profile, request, policy) = fixture();
        let root = vault.lock().unwrap().root.path().to_owned();
        let path = if alias {
            fs::hard_link(root.join(&request.path), root.join("tasks/Alias.md")).unwrap();
            "tasks/Alias.md"
        } else {
            &request.path
        };
        let client = vault.attach().unwrap();
        let editor = client.open_editor(path).unwrap();
        let mut journal = TaskLifecycleJournal::open(&profile, &vault).unwrap();
        let TaskRecoveryError::Partial { status, cause } = journal
            .submit_with(
                vault.begin_work().unwrap(),
                request.clone(),
                &policy,
                |stage| {
                    if stage == Checkpoint::Observed {
                        client.mark_dirty(editor, 1).unwrap();
                    }
                    Ok(())
                },
            )
            .unwrap_err()
        else {
            panic!("lost later partial")
        };
        let TaskRecoveryError::Service(error) = *cause else {
            panic!("lost dirty writer refusal")
        };
        assert_eq!(error.code(), ErrorCode::Busy);
        assert_eq!(
            fs::read_to_string(root.join(&request.path)).unwrap(),
            status.original.text
        );
        let daily = status.daily.unwrap();
        assert!(daily.text.contains("task-occurrence"));
        assert_eq!(
            fs::read_to_string(root.join(&daily.path)).unwrap(),
            daily.text
        );
        client.discard_editor(editor).unwrap();
        journal
            .retry(vault.begin_work().unwrap(), request.request_id)
            .unwrap();
        assert_eq!(
            fs::read_to_string(root.join(&daily.path)).unwrap(),
            daily.text
        );
        assert_eq!(
            fs::read_to_string(root.join(&request.path)).unwrap(),
            status.task_replacement
        );
    }
}

#[test]
fn daily_dirty_admission_precedes_quantitative_validation() {
    let (_temp, vault, profile, mut request, policy) = fixture();
    let daily = vault
        .begin_work()
        .unwrap()
        .ensure_default_daily_document("2026-08-20", &policy.daily, &policy.format)
        .unwrap();
    let client = vault.attach().unwrap();
    let editor = client.open_editor(&daily.path).unwrap();
    client.mark_dirty(editor, 1).unwrap();
    if let TaskLifecycleAction::Complete { tracked_value, .. } = &mut request.action {
        *tracked_value = Some(-1.0);
    }
    let mut journal = TaskLifecycleJournal::open(&profile, &vault).unwrap();
    let TaskRecoveryError::Partial { cause, status } = journal
        .submit(vault.begin_work().unwrap(), request, &policy)
        .unwrap_err()
    else {
        panic!("lost daily partial")
    };
    let TaskRecoveryError::Service(error) = *cause else {
        panic!("lost writer-admission precedence: {cause:?}")
    };
    assert_eq!(error.code(), ErrorCode::Busy);
    assert_eq!(status.daily.unwrap().text, daily.text);
    client.discard_editor(editor).unwrap();
}

#[test]
fn recovery_preserves_every_existing_markdown_daily_extension() {
    for extension in ["MD", "markdown", "MARKDOWN", "mD"] {
        let (_temp, vault, profile, request, policy) = fixture();
        let root = vault.lock().unwrap().root.path().to_owned();
        let path = format!("daily/Existing.{extension}");
        fs::create_dir(root.join("daily")).unwrap();
        fs::write(
            root.join(&path),
            "---\ntype: daily-note\ndate: 2026-08-20\n---\n\nHandwritten daily\n",
        )
        .unwrap();
        let mut journal = TaskLifecycleJournal::open(&profile, &vault).unwrap();
        let result = journal
            .submit(vault.begin_work().unwrap(), request, &policy)
            .unwrap();
        assert_eq!(result.disposition, TaskLifecycleDisposition::Completed);
        let daily = result.daily.unwrap();
        assert_eq!(daily.path, path);
        assert!(daily.text.contains("Handwritten daily"));
        assert_eq!(daily.text.matches("task-occurrence").count(), 1);
        assert!(!root.join("daily/2026-08-20.md").exists());
    }
}
