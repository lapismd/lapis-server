use lapis_app_core::services::{
    Runtime, Work,
    daily::{DailyDocumentOperationError, DailyDocumentSettings, DailyFormatContext},
};
use lapis_app_protocol::{ErrorCode, VaultPhase};
use serde_json::Value;
use std::{
    fs,
    sync::{Barrier, mpsc},
    thread,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

const DATE: &str = "2026-08-21";
fn context() -> DailyFormatContext {
    DailyFormatContext {
        locale: "en-US".into(),
        first_day: 7,
        minimal_days: 4,
    }
}
fn ensure(work: &Work) -> Result<lapis_app_protocol::TextSnapshot, DailyDocumentOperationError> {
    work.ensure_default_daily_document(DATE, &DailyDocumentSettings::default(), &context())
}
fn seed() -> String {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/daily-provider.json")).unwrap();
    fixture["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "default canonical seed")
        .unwrap()["expected"]["files"]["daily/2026-08-21.md"]
        .as_str()
        .unwrap()
        .into()
}
fn service(error: DailyDocumentOperationError) -> ErrorCode {
    match error {
        DailyDocumentOperationError::Service(error) => error.code(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn native_lookup_never_creates_and_ensure_keeps_exact_source_bytes_and_noop_mtime() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let work = vault.begin_work().unwrap();
    assert!(work.locate_default_daily_document(DATE).unwrap().is_none());
    assert!(!root.path().join("daily").exists());
    let first = ensure(&work).unwrap();
    assert_eq!(first.path, "daily/2026-08-21.md");
    assert_eq!(first.text, seed());
    let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1_500_000_000);
    fs::File::options()
        .write(true)
        .open(root.path().join(&first.path))
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(stamp))
        .unwrap();
    for _ in 0..2 {
        let repeated = ensure(&work).unwrap();
        assert_eq!(repeated.text, first.text);
        assert_eq!(repeated.version, first.version);
        assert_eq!(
            fs::metadata(root.path().join(&first.path))
                .unwrap()
                .modified()
                .unwrap(),
            stamp
        );
    }
}

#[test]
fn canonical_former_path_wins_over_new_or_invalid_creation_settings() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("archive")).unwrap();
    fs::write(root.path().join("archive/former.md"), seed()).unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    let editor = client.open_editor("archive/former.md").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    let work = vault.begin_work().unwrap();
    let settings = DailyDocumentSettings {
        folder: "../invalid".into(),
        date_format: "".into(),
    };
    let found = work
        .ensure_default_daily_document(DATE, &settings, &context())
        .unwrap();
    assert_eq!(found.path, "archive/former.md");
    assert_eq!(found.text, seed());
    assert!(!root.path().join("daily").exists());
    client.discard_editor(editor).unwrap();
}

#[test]
fn duplicates_and_occupied_unrelated_paths_remain_untouched() {
    for duplicate in [false, true] {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("daily")).unwrap();
        let original = if duplicate {
            seed()
        } else {
            "# Unrelated\n".into()
        };
        fs::write(root.path().join("daily/2026-08-21.md"), &original).unwrap();
        if duplicate {
            fs::write(root.path().join("another.md"), seed()).unwrap();
        }
        let runtime = Runtime::default();
        let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
        let work = vault.begin_work().unwrap();
        let DailyDocumentOperationError::Policy(error) = ensure(&work).unwrap_err() else {
            panic!("source policy identity lost")
        };
        assert!(error.message.contains(if duplicate {
            "Multiple daily notes"
        } else {
            "already exists"
        }));
        assert_eq!(
            fs::read_to_string(root.path().join("daily/2026-08-21.md")).unwrap(),
            original
        );
    }
}

#[test]
fn dirty_attached_and_detached_missing_targets_fence_creation_before_mkdir() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    let editor = client.open_editor("daily/2026-08-21.md").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    let work = vault.begin_work().unwrap();
    assert_eq!(service(ensure(&work).unwrap_err()), ErrorCode::Busy);
    drop(client);
    assert_eq!(service(ensure(&work).unwrap_err()), ErrorCode::Busy);
    assert!(!root.path().join("daily").exists());
    let recovered = vault.attach().unwrap();
    recovered.recover_editor(editor).unwrap();
    recovered.discard_editor(editor).unwrap();
    ensure(&work).unwrap();
}

#[test]
fn hidden_and_symlinked_sources_do_not_gain_lookup_or_creation_authority() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".hidden")).unwrap();
    fs::write(root.path().join(".hidden/canonical.md"), seed()).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("daily")).unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let work = vault.begin_work().unwrap();
    assert!(work.locate_default_daily_document(DATE).unwrap().is_none());
    assert_eq!(
        service(ensure(&work).unwrap_err()),
        ErrorCode::PermissionDenied
    );
    assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
    assert_eq!(
        fs::read_to_string(root.path().join(".hidden/canonical.md")).unwrap(),
        seed()
    );
}

#[test]
fn retained_snapshot_bytes_remain_bounded_before_default_creation() {
    let root = tempfile::tempdir().unwrap();
    fs::File::create(root.path().join("large.md"))
        .unwrap()
        .set_len(33 * 1024 * 1024)
        .unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let work = vault.begin_work().unwrap();
    assert_eq!(
        service(ensure(&work).unwrap_err()),
        ErrorCode::InvalidRequest
    );
    assert!(!root.path().join("daily").exists());
}

#[test]
fn concurrent_default_creation_keeps_one_canonical_target() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let first = vault.begin_work().unwrap();
    let second = vault.begin_work().unwrap();
    let barrier = Barrier::new(2);
    let results = thread::scope(|scope| {
        let run = |work: Work| {
            let barrier = &barrier;
            scope.spawn(move || {
                barrier.wait();
                ensure(&work)
            })
        };
        [run(first), run(second)].map(|task| task.join().unwrap())
    });
    assert!(results.iter().any(Result::is_ok));
    for result in results {
        match result {
            Ok(value) => assert_eq!(value.text, seed()),
            Err(error) => assert_eq!(service(error), ErrorCode::Conflict),
        }
    }
    assert_eq!(fs::read_dir(root.path().join("daily")).unwrap().count(), 1);
    assert_eq!(
        fs::read_to_string(root.path().join("daily/2026-08-21.md")).unwrap(),
        seed()
    );
}

#[test]
fn accepted_default_creation_finishes_during_close_after_detach() {
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
        ensure(&work).unwrap();
        assert!(closed_rx.recv_timeout(Duration::from_millis(20)).is_err());
        drop(work);
        closed_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    });
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
}

#[test]
fn replaced_physical_root_keeps_both_locations_unchanged() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let work = vault.begin_work().unwrap();
    let moved = root.path().with_extension("moved");
    fs::rename(root.path(), &moved).unwrap();
    fs::create_dir(root.path()).unwrap();
    assert_eq!(
        service(ensure(&work).unwrap_err()),
        ErrorCode::PermissionDenied
    );
    assert!(fs::read_dir(root.path()).unwrap().next().is_none());
    assert!(fs::read_dir(&moved).unwrap().next().is_none());
    fs::remove_dir(moved).unwrap();
}

#[test]
fn default_creation_does_not_publish_into_paths_excluded_from_its_inventory() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let work = vault.begin_work().unwrap();
    let settings = DailyDocumentSettings {
        folder: ".hidden".into(),
        date_format: "yyyy-MM-dd".into(),
    };
    assert_eq!(
        service(
            work.ensure_default_daily_document(DATE, &settings, &context())
                .unwrap_err()
        ),
        ErrorCode::Unsupported
    );
    assert!(!root.path().join(".hidden").exists());
}
