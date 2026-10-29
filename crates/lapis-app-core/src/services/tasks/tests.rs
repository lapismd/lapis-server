use super::{TaskWriteError, persistence};
use crate::services::Runtime;
use lapis_app_protocol::{ErrorCode, ExpectedVersion};
use uuid::Uuid;

#[test]
fn external_changes_between_transformation_and_persistence_remain_untouched() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Task.md"), "original").unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let work = vault.begin_work().unwrap();
    let expected = ExpectedVersion::Version {
        version: work.read_text("Task.md").unwrap().version,
    };
    let error = persistence::apply(&work, "Task.md", &expected, |raw| {
        assert_eq!(raw, "original");
        std::fs::write(root.path().join("Task.md"), "external").unwrap();
        Ok("transformed".into())
    })
    .unwrap_err();
    let TaskWriteError::Service(error) = error else {
        panic!("lost native error")
    };
    assert_eq!(error.code(), ErrorCode::Conflict);
    assert_eq!(work.read_text("Task.md").unwrap().text, "external");
}

#[test]
fn dirty_claims_created_during_transformation_block_changed_and_unchanged_results() {
    for replacement in ["original", "transformed"] {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("Task.md"), "original").unwrap();
        let runtime = Runtime::default();
        let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
        let work = vault.begin_work().unwrap();
        let client = vault.attach().unwrap();
        let expected = ExpectedVersion::Version {
            version: work.read_text("Task.md").unwrap().version,
        };
        let error = persistence::apply(&work, "Task.md", &expected, |_| {
            let editor = client.open_editor("Task.md").unwrap();
            client.mark_dirty(editor, 1).unwrap();
            Ok(replacement.into())
        })
        .unwrap_err();
        let TaskWriteError::Service(error) = error else {
            panic!("lost native error")
        };
        assert_eq!(error.code(), ErrorCode::Busy);
        assert_eq!(work.read_text("Task.md").unwrap().text, "original");
    }
}
