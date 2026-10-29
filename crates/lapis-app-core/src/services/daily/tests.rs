use super::{DailyDocumentOperationError, DailyDocumentSettings, DailyFormatContext, io};
use crate::services::Runtime;
use lapis_app_protocol::ErrorCode;
use std::{
    fs,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

fn service(error: DailyDocumentOperationError) -> ErrorCode {
    match error {
        DailyDocumentOperationError::Service(error) => error.code(),
        other => panic!("{other:?}"),
    }
}
fn context() -> DailyFormatContext {
    DailyFormatContext {
        locale: "en-US".into(),
        first_day: 7,
        minimal_days: 4,
    }
}

#[test]
fn same_size_same_mtime_external_change_between_prepare_and_creation_is_not_used() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("other.md");
    fs::write(&path, "first").unwrap();
    let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1_500_000_000);
    let file = fs::File::options().write(true).open(&path).unwrap();
    file.set_times(fs::FileTimes::new().set_modified(stamp))
        .unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let work = vault.begin_work().unwrap();
    let plan = io::prepare(
        &work,
        "2026-08-21",
        Some((&DailyDocumentSettings::default(), &context())),
    )
    .unwrap();
    fs::write(&path, "later").unwrap();
    file.set_times(fs::FileTimes::new().set_modified(stamp))
        .unwrap();
    assert_eq!(
        service(io::finish(&work, plan).unwrap_err()),
        ErrorCode::Conflict
    );
    assert_eq!(fs::read_to_string(path).unwrap(), "later");
    assert!(!root.path().join("daily").exists());
}

#[test]
fn a_new_canonical_file_after_inventory_fences_default_creation() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let work = vault.begin_work().unwrap();
    let plan = io::prepare(
        &work,
        "2026-08-21",
        Some((&DailyDocumentSettings::default(), &context())),
    )
    .unwrap();
    let raw = "---\ntype: daily-note\ndate: 2026-08-21\n---\n\nExternal\n";
    fs::write(root.path().join("external.md"), raw).unwrap();
    assert_eq!(
        service(io::finish(&work, plan).unwrap_err()),
        ErrorCode::Conflict
    );
    assert!(!root.path().join("daily").exists());
    assert_eq!(
        fs::read_to_string(root.path().join("external.md")).unwrap(),
        raw
    );
}

#[test]
fn an_editor_claim_created_after_preparation_fences_creation_before_mkdir() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let work = vault.begin_work().unwrap();
    let plan = io::prepare(
        &work,
        "2026-08-21",
        Some((&DailyDocumentSettings::default(), &context())),
    )
    .unwrap();
    let client = vault.attach().unwrap();
    let editor = client.open_editor("daily/2026-08-21.md").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    assert_eq!(
        service(io::finish(&work, plan).unwrap_err()),
        ErrorCode::Busy
    );
    assert!(!root.path().join("daily").exists());
    client.discard_editor(editor).unwrap();
}
