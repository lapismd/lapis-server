use lapis_app_core::services::Runtime;
use lapis_app_protocol::{ErrorCode, ExpectedVersion, VaultPhase};
use std::{
    fs,
    sync::{Arc, Barrier, mpsc},
    thread,
    time::Duration,
};
use uuid::Uuid;

fn seeded() -> (
    tempfile::TempDir,
    Runtime,
    Arc<lapis_app_core::services::VaultService>,
) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("note.md"), "original").unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), dir.path()).unwrap();
    (dir, runtime, vault)
}
fn expected(version: String) -> ExpectedVersion {
    ExpectedVersion::Version { version }
}

#[test]
fn detached_clients_do_not_stop_admitted_work_or_remove_the_vault() {
    let (_dir, runtime, vault) = seeded();
    let client = vault.attach().unwrap();
    let work = vault.begin_work().unwrap();
    let snapshot = client.read_text("note.md").unwrap();
    drop(client);
    assert_eq!(vault.status().unwrap().clients, 0);
    work.write_text("note.md", "finished", &expected(snapshot.version))
        .unwrap();
    drop(work);
    assert_eq!(
        runtime
            .get(vault.id())
            .unwrap()
            .attach()
            .unwrap()
            .read_text("note.md")
            .unwrap()
            .text,
        "finished"
    );
}

#[test]
fn competing_client_saves_serialize_and_one_stale_version_refuses() {
    let (_dir, _runtime, vault) = seeded();
    let first = vault.attach().unwrap();
    let second = vault.attach().unwrap();
    let version = first.read_text("note.md").unwrap().version;
    let barrier = Arc::new(Barrier::new(2));
    let results = thread::scope(|scope| {
        let run = |client: lapis_app_core::services::Client, text| {
            let barrier = barrier.clone();
            let version = version.clone();
            scope.spawn(move || {
                barrier.wait();
                client.write_text("note.md", text, &expected(version))
            })
        };
        let a = run(first, "first");
        let b = run(second, "second");
        [a.join().unwrap(), b.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .next()
            .unwrap()
            .code(),
        ErrorCode::Conflict
    );
}

#[test]
fn explicit_close_fences_clients_and_waits_for_existing_work_to_drain() {
    let (_dir, runtime, vault) = seeded();
    let client = vault.attach().unwrap();
    let work = vault.begin_work().unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (closed_tx, closed_rx) = mpsc::channel();
    thread::scope(|scope| {
        let selected = vault.clone();
        scope.spawn(move || {
            started_tx.send(()).unwrap();
            selected.close().unwrap();
            closed_tx.send(()).unwrap();
        });
        started_rx.recv().unwrap();
        while vault.status().unwrap().phase == VaultPhase::Open {
            thread::yield_now();
        }
        assert!(work.cancellation_requested());
        assert_eq!(vault.begin_work().err().unwrap().code(), ErrorCode::Closed);
        assert!(client.read_text("note.md").is_err());
        assert!(closed_rx.recv_timeout(Duration::from_millis(20)).is_err());
        let snapshot = work.read_text("note.md").unwrap();
        work.write_text("note.md", "final persistence", &expected(snapshot.version))
            .unwrap();
        drop(work);
        closed_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    });
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
    runtime.close_vault(vault.id()).unwrap();
}

#[test]
fn dirty_detached_editors_block_guarded_mutation_until_explicit_recovery() {
    let (_dir, _runtime, vault) = seeded();
    let client = vault.attach().unwrap();
    let editor = client.open_editor("note.md").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    drop(client);
    let work = vault.begin_work().unwrap();
    let original = work.read_text("note.md").unwrap();
    assert_eq!(
        work.write_text("note.md", "unsafe", &expected(original.version.clone()))
            .unwrap_err()
            .code(),
        ErrorCode::Busy
    );
    assert_eq!(vault.close().unwrap_err().code(), ErrorCode::Busy);
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Open);
    let reconnect = vault.attach().unwrap();
    reconnect.recover_editor(editor).unwrap();
    reconnect.discard_editor(editor).unwrap();
    work.write_text(
        "note.md",
        "explicitly resolved",
        &expected(original.version),
    )
    .unwrap();
}

#[test]
fn a_saved_acknowledgement_cannot_clear_newer_unsaved_edits() {
    let (_dir, _runtime, vault) = seeded();
    let client = vault.attach().unwrap();
    let editor = client.open_editor("note.md").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    client.mark_dirty(editor, 2).unwrap();
    let version = client.read_text("note.md").unwrap().version;
    assert!(!client.mark_saved(editor, 1, &version).unwrap());
    assert_eq!(vault.status().unwrap().dirty_editors, 1);
    assert!(client.mark_saved(editor, 2, &version).unwrap());
    assert_eq!(vault.status().unwrap().dirty_editors, 0);
}

#[test]
fn duplicate_physical_vaults_cannot_create_independent_writer_owners() {
    let (dir, runtime, vault) = seeded();
    let same = runtime.admit(vault.id(), dir.path()).unwrap();
    assert!(Arc::ptr_eq(&vault, &same));
    assert_eq!(
        runtime
            .admit(Uuid::new_v4(), dir.path())
            .err()
            .unwrap()
            .code(),
        ErrorCode::Busy
    );
    let other = tempfile::tempdir().unwrap();
    assert_eq!(
        runtime
            .admit(vault.id(), other.path())
            .err()
            .unwrap()
            .code(),
        ErrorCode::PermissionDenied
    );
}

#[test]
fn external_edits_and_unexpected_creation_preserve_the_external_bytes() {
    let (dir, _runtime, vault) = seeded();
    let client = vault.attach().unwrap();
    let version = client.read_text("note.md").unwrap().version;
    fs::write(dir.path().join("note.md"), "external").unwrap();
    assert_eq!(
        client
            .write_text("note.md", "stale", &expected(version))
            .unwrap_err()
            .code(),
        ErrorCode::Conflict
    );
    assert_eq!(
        client
            .write_text("note.md", "overwrite", &ExpectedVersion::Missing {})
            .unwrap_err()
            .code(),
        ErrorCode::Conflict
    );
    assert_eq!(client.read_text("note.md").unwrap().text, "external");
}

#[test]
fn shutdown_preflight_keeps_every_vault_live_when_one_has_unsaved_editors() {
    let (dir, runtime, vault) = seeded();
    let other = tempfile::tempdir().unwrap();
    let second = runtime.admit(Uuid::new_v4(), other.path()).unwrap();
    let client = vault.attach().unwrap();
    let editor = client.open_editor("note.md").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    assert_eq!(runtime.shutdown().unwrap_err().code(), ErrorCode::Busy);
    assert_eq!(second.status().unwrap().phase, VaultPhase::Open);
    client.discard_editor(editor).unwrap();
    runtime.shutdown().unwrap();
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
    assert!(runtime.admit(Uuid::new_v4(), dir.path()).is_err());
}

#[test]
fn reconnect_can_discover_dirty_claims_and_cannot_take_attached_editors() {
    let (_dir, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    let editor = owner.open_editor("note.md").unwrap();
    owner.mark_dirty(editor, 2).unwrap();
    let observer = vault.attach().unwrap();
    let claims = observer.editor_claims().unwrap();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].editor_id, editor);
    assert!(!claims[0].detached);
    assert!(observer.recover_editor(editor).is_err());
    drop(owner);
    let claims = observer.editor_claims().unwrap();
    assert!(claims[0].detached);
    assert_eq!(claims[0].revision, 2);
    observer.recover_editor(claims[0].editor_id).unwrap();
    observer.discard_editor(editor).unwrap();
    assert!(observer.editor_claims().unwrap().is_empty());
}

#[test]
fn another_client_or_native_work_cannot_overwrite_a_dirty_editor() {
    let (_dir, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    let other = vault.attach().unwrap();
    let editor = owner.open_editor("note.md").unwrap();
    owner.mark_dirty(editor, 1).unwrap();
    let version = owner.read_text("note.md").unwrap().version;
    let expected = expected(version);
    assert_eq!(
        other
            .write_text("note.md", "unsafe", &expected)
            .unwrap_err()
            .code(),
        ErrorCode::Busy
    );
    let work = vault.begin_work().unwrap();
    assert_eq!(
        work.write_text("note.md", "unsafe", &expected)
            .unwrap_err()
            .code(),
        ErrorCode::Busy
    );
    let saved = owner
        .write_text("note.md", "owned save", &expected)
        .unwrap();
    assert!(owner.mark_saved(editor, 1, &saved).unwrap());
    assert_eq!(other.read_text("note.md").unwrap().text, "owned save");
}
