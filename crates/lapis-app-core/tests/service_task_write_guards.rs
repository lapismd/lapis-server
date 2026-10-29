use lapis_app_core::services::{Runtime, TaskWriteError, VaultService, Work};
use lapis_app_protocol::{ErrorCode, ExpectedVersion, VaultPhase};
use lapis_task_services::{TaskMutation, TaskOccurrenceMaterialization};
use serde_json::Value;
use std::{
    fs,
    sync::{Arc, Barrier, mpsc},
    thread,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

fn case(name: &str) -> Value {
    let fixtures: Value = serde_json::from_str(include_str!("fixtures/task-writes.json")).unwrap();
    fixtures["cases"][name].clone()
}
fn seeded(name: &str) -> (tempfile::TempDir, Runtime, Arc<VaultService>, Value) {
    let root = tempfile::tempdir().unwrap();
    let case = case(name);
    let path = case["path"].as_str().unwrap();
    fs::create_dir_all(root.path().join(path).parent().unwrap()).unwrap();
    fs::write(root.path().join(path), case["raw"].as_str().unwrap()).unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    (root, runtime, vault, case)
}
fn version(work: &Work, path: &str) -> ExpectedVersion {
    ExpectedVersion::Version {
        version: work.read_text(path).unwrap().version,
    }
}
fn mutation(case: &Value) -> TaskMutation {
    serde_json::from_value(case["actions"][0].clone()).unwrap()
}
fn input(case: &Value) -> TaskOccurrenceMaterialization {
    serde_json::from_value(case["input"].clone()).unwrap()
}
fn service(error: TaskWriteError) -> ErrorCode {
    match error {
        TaskWriteError::Service(error) => error.code(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn repeated_materialization_preserves_mtime_and_content_version() {
    let (root, _runtime, vault, case) = seeded("noop");
    let work = vault.begin_work().unwrap();
    let path = case["path"].as_str().unwrap();
    let before = work.read_text(path).unwrap();
    let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1_500_000_000);
    let file = fs::File::options()
        .write(true)
        .open(root.path().join(path))
        .unwrap();
    file.set_times(fs::FileTimes::new().set_modified(stamp))
        .unwrap();
    for _ in 0..2 {
        let after = work
            .materialize_task_occurrence(path, &version(&work, path), &input(&case))
            .unwrap();
        assert_eq!(after.text, before.text);
        assert_eq!(after.version, before.version);
        assert_eq!(
            fs::metadata(root.path().join(path))
                .unwrap()
                .modified()
                .unwrap(),
            stamp
        );
    }
}

#[test]
fn a_noop_still_rejects_a_stale_content_version() {
    let (root, _runtime, vault, case) = seeded("noop");
    let work = vault.begin_work().unwrap();
    let path = case["path"].as_str().unwrap();
    let expected = version(&work, path);
    let external = format!("{}\nExternal", case["raw"].as_str().unwrap());
    fs::write(root.path().join(path), &external).unwrap();
    assert_eq!(
        service(
            work.materialize_task_occurrence(path, &expected, &input(&case))
                .unwrap_err()
        ),
        ErrorCode::Conflict
    );
    assert_eq!(
        fs::read_to_string(root.path().join(path)).unwrap(),
        external
    );
}

#[test]
fn dirty_attached_and_detached_editors_block_task_writes_and_noops() {
    for family in ["mutation", "noop"] {
        let (root, _runtime, vault, case) = seeded(family);
        let path = case["path"].as_str().unwrap();
        let client = vault.attach().unwrap();
        let editor = client.open_editor(path).unwrap();
        client.mark_dirty(editor, 1).unwrap();
        let work = vault.begin_work().unwrap();
        let expected = version(&work, path);
        let run = || {
            if family == "mutation" {
                work.mutate_task(path, &expected, &mutation(&case))
            } else {
                work.materialize_task_occurrence(path, &expected, &input(&case))
            }
        };
        assert_eq!(service(run().unwrap_err()), ErrorCode::Busy);
        drop(client);
        assert_eq!(service(run().unwrap_err()), ErrorCode::Busy);
        assert_eq!(
            fs::read_to_string(root.path().join(path)).unwrap(),
            case["raw"].as_str().unwrap()
        );
        let recovered = vault.attach().unwrap();
        recovered.recover_editor(editor).unwrap();
        recovered.discard_editor(editor).unwrap();
        run().unwrap();
    }
}

#[test]
fn hardlink_alias_dirty_claims_preserve_both_names() {
    let (root, _runtime, vault, case) = seeded("mutation");
    let path = case["path"].as_str().unwrap();
    fs::hard_link(root.path().join(path), root.path().join("alias.md")).unwrap();
    let client = vault.attach().unwrap();
    let editor = client.open_editor("alias.md").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    let work = vault.begin_work().unwrap();
    assert_eq!(
        service(
            work.mutate_task(path, &version(&work, path), &mutation(&case))
                .unwrap_err()
        ),
        ErrorCode::Busy
    );
    for name in [path, "alias.md"] {
        assert_eq!(
            fs::read_to_string(root.path().join(name)).unwrap(),
            case["raw"].as_str().unwrap()
        );
    }
}

#[test]
fn replaced_physical_root_preserves_admitted_and_replacement_files() {
    let (root, _runtime, vault, case) = seeded("mutation");
    let work = vault.begin_work().unwrap();
    let path = case["path"].as_str().unwrap();
    let expected = version(&work, path);
    let moved = root.path().with_extension("moved");
    fs::rename(root.path(), &moved).unwrap();
    fs::create_dir_all(root.path().join(path).parent().unwrap()).unwrap();
    fs::write(root.path().join(path), "replacement").unwrap();
    assert_eq!(
        service(
            work.mutate_task(path, &expected, &mutation(&case))
                .unwrap_err()
        ),
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        fs::read_to_string(moved.join(path)).unwrap(),
        case["raw"].as_str().unwrap()
    );
    assert_eq!(
        fs::read_to_string(root.path().join(path)).unwrap(),
        "replacement"
    );
    fs::remove_dir_all(moved).unwrap();
}

#[test]
fn competing_task_writes_share_the_existing_content_version_gate() {
    let (_root, _runtime, vault, case) = seeded("mutation");
    let path = case["path"].as_str().unwrap();
    let first = vault.begin_work().unwrap();
    let second = vault.begin_work().unwrap();
    let expected = version(&first, path);
    let action = mutation(&case);
    let barrier = Barrier::new(2);
    let results = thread::scope(|scope| {
        let run = |work: Work| {
            let barrier = &barrier;
            let expected = &expected;
            let action = &action;
            scope.spawn(move || {
                barrier.wait();
                work.mutate_task(path, expected, action)
            })
        };
        [run(first), run(second)].map(|task| task.join().unwrap())
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let error = results.into_iter().find_map(Result::err).unwrap();
    assert_eq!(service(error), ErrorCode::Conflict);
    assert_eq!(
        vault.begin_work().unwrap().read_text(path).unwrap().text,
        case["expected"]["value"]["raw"].as_str().unwrap()
    );
}

#[test]
fn accepted_task_work_finishes_during_close_after_client_detach() {
    let (_root, _runtime, vault, case) = seeded("mutation");
    let client = vault.attach().unwrap();
    let work = vault.begin_work().unwrap();
    let path = case["path"].as_str().unwrap();
    let expected = version(&work, path);
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
        assert!(closed_rx.recv_timeout(Duration::from_millis(20)).is_err());
        work.mutate_task(path, &expected, &mutation(&case)).unwrap();
        drop(work);
        closed_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    });
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
}

#[test]
fn complete_qualified_transform_errors_preserve_identity_and_disk_bytes() {
    for family in ["error", "checklistError", "duplicate"] {
        let (root, _runtime, vault, case) = seeded(family);
        let work = vault.begin_work().unwrap();
        let path = case["path"].as_str().unwrap();
        let expected = version(&work, path);
        let result = if family == "duplicate" {
            work.materialize_task_occurrence(path, &expected, &input(&case))
        } else {
            work.mutate_task(path, &expected, &mutation(&case))
        };
        let TaskWriteError::Transform(error) = result.unwrap_err() else {
            panic!("lost Task error identity")
        };
        assert_eq!(
            serde_json::to_value(error).unwrap(),
            case["expected"]["error"]
        );
        assert_eq!(
            fs::read_to_string(root.path().join(path)).unwrap(),
            case["raw"].as_str().unwrap()
        );
    }
}

#[test]
fn existing_file_daily_contract_refuses_missing_targets_and_wrong_dates() {
    let (root, _runtime, vault, case) = seeded("noop");
    let work = vault.begin_work().unwrap();
    let path = case["path"].as_str().unwrap();
    assert_eq!(
        service(
            work.materialize_task_occurrence(path, &ExpectedVersion::Missing {}, &input(&case))
                .unwrap_err()
        ),
        ErrorCode::InvalidRequest
    );
    let malformed = ExpectedVersion::Version {
        version: "invalid".into(),
    };
    assert_eq!(
        service(
            work.materialize_task_occurrence(path, &malformed, &input(&case))
                .unwrap_err()
        ),
        ErrorCode::InvalidRequest
    );
    let expected = version(&work, path);
    fs::remove_file(root.path().join(path)).unwrap();
    assert_eq!(
        service(
            work.materialize_task_occurrence(path, &expected, &input(&case))
                .unwrap_err()
        ),
        ErrorCode::Conflict
    );
    assert!(!root.path().join(path).exists());
    fs::write(root.path().join(path), "# Ordinary note\n").unwrap();
    let result = work.materialize_task_occurrence(path, &version(&work, path), &input(&case));
    assert!(matches!(result, Err(TaskWriteError::Transform(_))));
    assert_eq!(
        fs::read_to_string(root.path().join(path)).unwrap(),
        "# Ordinary note\n"
    );
}
