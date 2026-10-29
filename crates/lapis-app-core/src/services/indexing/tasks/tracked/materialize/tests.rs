use super::*;
use crate::services::{Runtime, TaskLifecyclePolicy, VaultService, daily::*};
use lapis_app_protocol::VaultPhase;
use lapis_task_services::{parse_task_document, scan_task_occurrence_observations};
use serde_json::Value;
use std::{
    fs,
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

fn fixture() -> (
    tempfile::TempDir,
    Arc<VaultService>,
    ParsedTaskDocument,
    TrackedTaskProvider,
) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("vault");
    fs::create_dir(&root).unwrap();
    let corpus: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/task-tracked-today.json"
    )))
    .unwrap();
    let case = &corpus["cases"][0];
    let raw = case["files"]["Habit.md"].as_str().unwrap();
    fs::write(root.join("Habit.md"), raw).unwrap();
    let doc = parse_task_document(raw, "Habit.md").unwrap();
    let vault = Runtime::default().admit(Uuid::new_v4(), &root).unwrap();
    let provider = TrackedTaskProvider::Default(TaskLifecyclePolicy {
        day_parts: serde_json::from_value(case["expected"]["settings"].clone()).unwrap(),
        daily: DailyDocumentSettings::default(),
        format: DailyFormatContext {
            locale: "en-US".into(),
            first_day: 7,
            minimal_days: 4,
        },
    });
    (temp, vault, doc, provider)
}

#[test]
fn tracked_stale_original_is_rejected_before_any_daily_creation() {
    let (temp, vault, doc, provider) = fixture();
    let root = temp.path().join("vault");
    fs::write(root.join("Habit.md"), "# Handwritten replacement\n").unwrap();
    let work = vault.begin_work().unwrap();
    let error = apply(&work, &doc, "2026-08-20", &provider).err().unwrap();
    assert!(matches!(error, TrackedTaskError::Source(_)));
    assert!(!root.join("daily").exists());
    assert_eq!(
        fs::read_to_string(root.join("Habit.md")).unwrap(),
        "# Handwritten replacement\n"
    );
}

#[test]
fn tracked_ensured_seed_survives_later_dirty_alias_and_cold_retry_is_idempotent() {
    let (temp, vault, doc, provider) = fixture();
    let root = temp.path().join("vault");
    let id = vault.id();
    let work = vault.begin_work().unwrap();
    let client = vault.attach().unwrap();
    let mut dirty_editor = None;
    let error = apply_with(&work, &doc, "2026-08-20", &provider, |daily| {
        fs::hard_link(root.join(&daily.path), root.join("Alias.txt")).unwrap();
        let editor = client.open_editor("Alias.txt").unwrap();
        client.mark_dirty(editor, 1).unwrap();
        dirty_editor = Some(editor);
    })
    .err()
    .unwrap();
    assert!(matches!(error, TrackedTaskError::Daily(_)));
    let seed = work
        .locate_default_daily_document("2026-08-20")
        .unwrap()
        .unwrap();
    assert!(!seed.text.contains("task-occurrence"));
    assert_eq!(fs::read_to_string(root.join("Habit.md")).unwrap(), doc.raw);
    drop(client);
    let client = vault.attach().unwrap();
    client.recover_editor(dirty_editor.unwrap()).unwrap();
    client.discard_editor(dirty_editor.unwrap()).unwrap();
    drop(client);
    fs::remove_file(root.join("Alias.txt")).unwrap();
    drop(work);
    vault.close().unwrap();
    let next = Runtime::default().admit(id, &root).unwrap();
    let work = next.begin_work().unwrap();
    let first = apply(&work, &doc, "2026-08-20", &provider).unwrap();
    assert!(first.changed);
    let version = first.write.document.version.clone();
    let second = apply(&work, &doc, "2026-08-20", &provider).unwrap();
    assert!(!second.changed);
    assert_eq!(second.write.document.version, version);
    assert_eq!(
        scan_task_occurrence_observations(&second.write.document.text, &second.write.document.path)
            .len(),
        1
    );
    assert_eq!(fs::read_to_string(root.join("Habit.md")).unwrap(), doc.raw);
}

#[test]
fn tracked_accepted_daily_mutation_finishes_after_detach_and_during_close_drain() {
    let (temp, vault, doc, provider) = fixture();
    let client = vault.attach().unwrap();
    let work = vault.begin_work().unwrap();
    let mut closing = None;
    let applied = apply_with(&work, &doc, "2026-08-20", &provider, |_| {
        drop(client);
        let next = vault.clone();
        closing = Some(std::thread::spawn(move || next.close()));
        let deadline = Instant::now() + Duration::from_secs(5);
        while vault.status().unwrap().phase != VaultPhase::Closing {
            assert!(
                Instant::now() < deadline,
                "close must begin draining accepted Work"
            );
            std::thread::yield_now();
        }
        assert!(vault.begin_work().is_err());
        assert_eq!(vault.status().unwrap().clients, 0);
    })
    .unwrap();
    assert_eq!(
        scan_task_occurrence_observations(
            &applied.write.document.text,
            &applied.write.document.path
        )
        .len(),
        1
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("vault/Habit.md")).unwrap(),
        doc.raw
    );
    drop(work);
    closing.unwrap().join().unwrap().unwrap();
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
    assert_eq!(vault.status().unwrap().work, 0);
}
