use lapis_app_core::{
    context::bootstrap::{BootstrapInput, prepare_bootstrap},
    services::{Error, Runtime, VaultService},
};
use lapis_app_protocol::{ErrorCode, VaultPhase};
use std::{
    cell::Cell,
    fs, io,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

fn seeded() -> (tempfile::TempDir, Runtime, Arc<VaultService>) {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".lapis")).unwrap();
    fs::write(root.path().join(".lapis/AGENTS.md"), "Current rules").unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    (root, runtime, vault)
}

#[test]
fn cancellation_before_or_during_read_never_delivers_partial_bootstrap() {
    for before in [true, false] {
        let cancelled = Cell::new(before);
        let mut reads = Vec::new();
        let mut reader = |path: &str| {
            reads.push(path.to_owned());
            cancelled.set(true);
            Ok(Some("Never delivered".into()))
        };
        let failure = prepare_bootstrap(&BootstrapInput::default(), Some(&mut reader), || {
            cancelled.get()
        })
        .unwrap_err();
        assert_eq!(failure.code(), ErrorCode::Closed);
        assert_eq!(reads.len(), usize::from(!before));
    }
}

#[test]
fn admitted_cancellation_after_read_preserves_files_and_work_ownership() {
    let (root, _runtime, vault) = seeded();
    let work = vault.begin_work().unwrap();
    let calls = Cell::new(0);
    let error = work
        .bootstrap_context(&BootstrapInput::default(), || {
            calls.set(calls.get() + 1);
            calls.get() == 3
        })
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::Closed);
    assert_eq!(calls.get(), 3);
    assert_eq!(vault.status().unwrap().work, 1);
    assert_eq!(
        fs::read_to_string(root.path().join(".lapis/AGENTS.md")).unwrap(),
        "Current rules"
    );
    assert_eq!(root.path().read_dir().unwrap().count(), 1);
}

#[test]
fn close_fences_bootstrap_delivery_and_drains_only_the_existing_lease() {
    let (root, _runtime, vault) = seeded();
    let work = vault.begin_work().unwrap();
    let (closed_tx, closed_rx) = mpsc::channel();
    thread::scope(|scope| {
        let closing = vault.clone();
        scope.spawn(move || closed_tx.send(closing.close()).unwrap());
        let deadline = Instant::now() + Duration::from_secs(2);
        while vault.status().unwrap().phase == VaultPhase::Open {
            assert!(Instant::now() < deadline, "close did not fence admission");
            thread::yield_now();
        }
        assert_eq!(
            work.bootstrap_context(&BootstrapInput::default(), || false)
                .unwrap_err()
                .code(),
            ErrorCode::Closed
        );
        assert_eq!(vault.status().unwrap().work, 1);
        assert!(closed_rx.try_recv().is_err());
        drop(work);
        closed_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
    });
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
    assert_eq!(
        fs::read_to_string(root.path().join(".lapis/AGENTS.md")).unwrap(),
        "Current rules"
    );
}

#[test]
fn admitted_scope_rejects_nonportable_traversal_and_non_directories() {
    let (root, _runtime, vault) = seeded();
    fs::write(root.path().join("note.md"), "A file, not a scope").unwrap();
    let work = vault.begin_work().unwrap();
    for scope in ["../x", "/x", "a//x", "a/./b", "C:x", "a\\x"] {
        let input = BootstrapInput {
            scope_dir: scope.into(),
            ..Default::default()
        };
        assert_eq!(
            work.bootstrap_context(&input, || false).unwrap_err().code(),
            ErrorCode::InvalidRequest,
            "{scope}"
        );
    }
    let input = BootstrapInput {
        scope_dir: "note.md".into(),
        ..Default::default()
    };
    assert!(work.bootstrap_context(&input, || false).is_err());
}

#[test]
fn missing_ancestor_files_are_absent_but_invalid_native_files_are_errors() {
    let (root, _runtime, vault) = seeded();
    let path = root.path().join(".lapis/AGENTS.md");
    fs::remove_file(&path).unwrap();
    let work = vault.begin_work().unwrap();
    let value = work
        .bootstrap_context(&BootstrapInput::default(), || false)
        .unwrap();
    assert!(value.bootstrap.folder_instructions.is_empty());
    fs::write(&path, [0xff, 0xfe]).unwrap();
    assert_eq!(
        work.bootstrap_context(&BootstrapInput::default(), || false)
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(fs::read(&path).unwrap(), [0xff, 0xfe]);
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(
        work.bootstrap_context(&BootstrapInput::default(), || false)
            .is_err()
    );
    assert!(path.is_dir());
}

#[test]
fn reader_permission_errors_are_not_treated_as_missing_instructions() {
    let mut reads = Vec::new();
    let mut reader = |path: &str| {
        reads.push(path.to_owned());
        if reads.len() == 1 {
            Ok(None)
        } else {
            Err(Error::from(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "/private/vault/rules",
            )))
        }
    };
    let input = BootstrapInput {
        scope_dir: "Project".into(),
        ..Default::default()
    };
    let error = prepare_bootstrap(&input, Some(&mut reader), || false).unwrap_err();
    assert_eq!(error.code(), ErrorCode::PermissionDenied);
    assert_eq!(reads, [".lapis/AGENTS.md", "Project/.lapis/AGENTS.md"]);
    assert!(!error.to_string().contains("/private/"));
}

#[test]
fn path_screening_precedes_truncation_and_total_budget_stops_later_reads() {
    let mut reads = Vec::new();
    let mut reader = |path: &str| {
        reads.push(path.to_owned());
        Ok(Some(if reads.len() == 1 {
            format!("{} /Users/private/secret", "x".repeat(10_001))
        } else {
            "📝".repeat(5_001)
        }))
    };
    let input = BootstrapInput {
        scope_dir: "a/b/c/d".into(),
        ..Default::default()
    };
    let value = prepare_bootstrap(&input, Some(&mut reader), || false).unwrap();
    assert_eq!(reads.len(), 4);
    assert_eq!(reads[3], "a/b/c/.lapis/AGENTS.md");
    let folders = &value.bootstrap.folder_instructions;
    assert_eq!(folders[0].omitted.as_deref(), Some("path-bearing"));
    assert_eq!(folders[0].text, "");
    assert_eq!(
        folders
            .iter()
            .map(|entry| entry.text.encode_utf16().count())
            .collect::<Vec<_>>(),
        [0, 10_000, 10_000, 5_000]
    );
    assert!(value.bootstrap.truncated);
    assert!(!value.text.contains("/Users/private"));
}

#[test]
fn replaced_physical_root_before_or_after_read_never_delivers_context() {
    for after_read in [false, true] {
        let (root, _runtime, vault) = seeded();
        let retained = tempfile::tempdir().unwrap();
        let work = vault.begin_work().unwrap();
        let replace = || {
            fs::rename(root.path(), retained.path().join("original")).unwrap();
            fs::create_dir_all(root.path().join(".lapis")).unwrap();
            fs::write(root.path().join(".lapis/AGENTS.md"), "Replacement").unwrap();
        };
        if !after_read {
            replace();
        }
        let calls = Cell::new(0);
        let failure = work
            .bootstrap_context(&BootstrapInput::default(), || {
                calls.set(calls.get() + 1);
                if after_read && calls.get() == 3 {
                    replace();
                }
                false
            })
            .unwrap_err();
        assert_eq!(failure.code(), ErrorCode::PermissionDenied);
        assert_eq!(
            fs::read_to_string(retained.path().join("original/.lapis/AGENTS.md")).unwrap(),
            "Current rules"
        );
        assert_eq!(
            fs::read_to_string(root.path().join(".lapis/AGENTS.md")).unwrap(),
            "Replacement"
        );
    }
}

#[test]
#[cfg(unix)]
fn linked_scope_and_instruction_files_cannot_expand_bootstrap_authority() {
    use std::os::unix::fs::symlink;
    let (root, _runtime, vault) = seeded();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), "Outside rules").unwrap();
    symlink(outside.path(), root.path().join("escape")).unwrap();
    let work = vault.begin_work().unwrap();
    let scope = BootstrapInput {
        scope_dir: "escape".into(),
        ..Default::default()
    };
    assert_eq!(
        work.bootstrap_context(&scope, || false).unwrap_err().code(),
        ErrorCode::PermissionDenied
    );
    let path = root.path().join(".lapis/AGENTS.md");
    fs::remove_file(&path).unwrap();
    symlink(outside.path().join("secret"), &path).unwrap();
    assert_eq!(
        work.bootstrap_context(&BootstrapInput::default(), || false)
            .unwrap_err()
            .code(),
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        fs::read_to_string(outside.path().join("secret")).unwrap(),
        "Outside rules"
    );
}
