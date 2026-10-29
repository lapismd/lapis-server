use lapis_app_core::services::{Runtime, VaultService};
use lapis_app_protocol::{ErrorCode, VaultPhase};
use std::{
    cell::Cell,
    fs,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

fn seeded() -> (tempfile::TempDir, Runtime, Arc<VaultService>) {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("note.md"), "Fresh café 📝").unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    (root, runtime, vault)
}

#[test]
fn admitted_reads_preserve_exact_hashes_duplicates_and_disk_bytes() {
    let (root, _runtime, vault) = seeded();
    fs::write(root.path().join("empty.md"), "").unwrap();
    let client = vault.attach().unwrap();
    let editor = client.open_editor("note.md").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    let work = vault.begin_work().unwrap();
    let paths = ["note.md".into(), "empty.md".into(), "note.md".into()];
    let value = work.attachment_context(&paths, || false).unwrap();
    assert_eq!(value.sources.len(), 3);
    assert_eq!(value.sources[0], value.sources[2]);
    assert_eq!(value.blocks[0], value.blocks[2]);
    assert_eq!(value.sources[0].bytes, 16);
    assert_eq!(
        value.sources[0].sha256,
        "4a45a7b36594350bd473e7da1ef19bb0dceea5444e1adfe57fae4c61f5446b31"
    );
    assert_eq!(value.sources[1].bytes, 0);
    assert_eq!(
        value.sources[1].sha256,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(value.blocks[1].content, "File: empty.md\n\n");
    assert_eq!(vault.status().unwrap().dirty_editors, 1);
    assert_eq!(
        fs::read_to_string(root.path().join("note.md")).unwrap(),
        "Fresh café 📝"
    );
    assert_eq!(root.path().read_dir().unwrap().count(), 2);
    client.discard_editor(editor).unwrap();
}

#[test]
fn admitted_reads_enforce_utf8_byte_and_aggregate_limits() {
    let (root, _runtime, vault) = seeded();
    let boundary = "📝".repeat(16_384);
    fs::write(root.path().join("boundary.md"), &boundary).unwrap();
    fs::write(root.path().join("one.md"), "x").unwrap();
    let work = vault.begin_work().unwrap();
    let pair = ["boundary.md".into(), "boundary.md".into()];
    let value = work.attachment_context(&pair, || false).unwrap();
    assert_eq!(
        value
            .sources
            .iter()
            .map(|source| source.bytes)
            .sum::<usize>(),
        131_072
    );
    let mut excessive = pair.to_vec();
    excessive.push("one.md".into());
    assert_eq!(
        work.attachment_context(&excessive, || false)
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    fs::write(root.path().join("boundary.md"), format!("{boundary}x")).unwrap();
    let failure = work.attachment_context(&pair[..1], || false).unwrap_err();
    assert_eq!(failure.code(), ErrorCode::InvalidRequest);
    assert_eq!(
        failure.to_string(),
        "File context exceeds 64 KiB per file or 128 KiB per message"
    );
    assert_eq!(
        fs::metadata(root.path().join("boundary.md")).unwrap().len(),
        65_537
    );
}

#[test]
fn admitted_reads_refuse_missing_unsafe_invalid_utf8_and_non_regular_files() {
    let (root, _runtime, vault) = seeded();
    fs::write(root.path().join("binary.md"), [0xff, 0xfe]).unwrap();
    fs::create_dir(root.path().join("folder")).unwrap();
    let work = vault.begin_work().unwrap();
    for path in [
        "../note.md",
        "/note.md",
        "a//note.md",
        "a/./b",
        "C:note.md",
        "a\\note.md",
        "",
    ] {
        assert_eq!(
            work.attachment_context(&[path.into()], || false)
                .unwrap_err()
                .code(),
            ErrorCode::InvalidRequest,
            "{path:?}"
        );
    }
    let missing = work
        .attachment_context(&["missing.md".into()], || false)
        .unwrap_err();
    assert_eq!(missing.code(), ErrorCode::NotFound);
    assert_eq!(
        missing.to_string(),
        "Attached file is unavailable: missing.md"
    );
    let binary = work
        .attachment_context(&["binary.md".into()], || false)
        .unwrap_err();
    assert_eq!(binary.code(), ErrorCode::InvalidRequest);
    assert_eq!(binary.to_string(), "File is not valid UTF-8");
    assert!(
        work.attachment_context(&["folder".into()], || false)
            .is_err()
    );
    assert_eq!(
        work.attachment_context(&vec!["note.md".into(); 9], || false)
            .unwrap_err()
            .code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        fs::read(root.path().join("binary.md")).unwrap(),
        [0xff, 0xfe]
    );
}

#[test]
fn cancellation_refuses_late_context_without_modifying_saved_files() {
    let (root, _runtime, vault) = seeded();
    let work = vault.begin_work().unwrap();
    let paths = ["note.md".into()];
    assert_eq!(
        work.attachment_context(&paths, || true).unwrap_err().code(),
        ErrorCode::Closed
    );
    let calls = Cell::new(0);
    let failure = work
        .attachment_context(&paths, || {
            calls.set(calls.get() + 1);
            calls.get() == 2
        })
        .unwrap_err();
    assert_eq!(calls.get(), 2);
    assert_eq!(failure.code(), ErrorCode::Closed);
    assert_eq!(failure.to_string(), "This operation was aborted");
    assert_eq!(
        fs::read_to_string(root.path().join("note.md")).unwrap(),
        "Fresh café 📝"
    );
    assert_eq!(vault.status().unwrap().work, 1);
}

#[test]
fn changed_physical_admission_never_returns_replacement_context() {
    let (root, _runtime, vault) = seeded();
    let retained = tempfile::tempdir().unwrap();
    let work = vault.begin_work().unwrap();
    fs::rename(root.path(), retained.path().join("original")).unwrap();
    fs::create_dir(root.path()).unwrap();
    fs::write(root.path().join("note.md"), "replacement").unwrap();
    assert_eq!(
        work.attachment_context(&["note.md".into()], || false)
            .unwrap_err()
            .code(),
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        fs::read_to_string(retained.path().join("original/note.md")).unwrap(),
        "Fresh café 📝"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("note.md")).unwrap(),
        "replacement"
    );
}

#[test]
#[cfg(unix)]
fn linked_files_and_directories_cannot_expand_attachment_authority() {
    use std::os::unix::fs::symlink;
    let (root, _runtime, vault) = seeded();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.md"), "outside").unwrap();
    symlink(outside.path(), root.path().join("escape")).unwrap();
    symlink("note.md", root.path().join("alias.md")).unwrap();
    let work = vault.begin_work().unwrap();
    for path in ["escape/secret.md", "alias.md"] {
        let error = work
            .attachment_context(&[path.into()], || false)
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::PermissionDenied);
        assert!(!error.to_string().contains("outside"));
    }
    assert_eq!(
        fs::read_to_string(outside.path().join("secret.md")).unwrap(),
        "outside"
    );
}

#[test]
fn close_cancels_context_and_drains_its_existing_work_lease() {
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
            work.attachment_context(&["note.md".into()], || false)
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
        fs::read_to_string(root.path().join("note.md")).unwrap(),
        "Fresh café 📝"
    );
    assert_eq!(root.path().read_dir().unwrap().count(), 1);
}
