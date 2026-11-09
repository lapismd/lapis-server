use super::*;
use crate::services::Runtime;
use std::{fs, sync::Arc, thread};

fn read(vault: Uuid) -> ContentRequest {
    ContentRequest {
        api_version: 1,
        request_id: Uuid::new_v4(),
        command: ContentCommand::ReadNote {
            vault_id: vault,
            path: "note.md".into(),
        },
    }
}

fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "synchronization deadline");
        thread::yield_now();
    }
}

#[test]
fn revoke_fences_a_reader_waiting_for_the_native_vault_lock_and_drains_it() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("note.md"), "saved").unwrap();
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, directory.path()).unwrap();
    let session = Arc::new(
        ContentSession::read_only(
            vec![ContentVault {
                name: "test".into(),
                client: vault.attach().unwrap(),
            }],
            Duration::from_secs(60),
        )
        .unwrap(),
    );
    let native_guard = vault.lock().unwrap();
    thread::scope(|scope| {
        let reader = scope.spawn(|| session.execute(read(id)));
        wait_for(|| session.state.try_lock().is_err());
        let revoker = scope.spawn(|| session.revoke());
        wait_for(|| session.revoked.load(Ordering::Acquire));
        drop(native_guard);
        let ContentOutcome::Error { error } = reader.join().unwrap().outcome else {
            panic!("revoked read succeeded")
        };
        assert_eq!(error.code, ContentErrorCode::Unauthenticated);
        revoker.join().unwrap().unwrap();
    });
    assert_eq!(vault.status().unwrap().clients, 0);
}

#[test]
fn cursor_storage_is_bounded_and_evicted_handles_explicitly_conflict() {
    let mut cursors = pagination::Cursors::default();
    let items: Vec<usize> = (0..140).collect();
    let (_, first) = cursors
        .page(
            pagination::Key::Vaults,
            items.clone(),
            &ContentPageQuery {
                limit: 1,
                cursor: None,
            },
        )
        .unwrap();
    let mut cursor = first.clone();
    for _ in 0..130 {
        let (_, next) = cursors
            .page(
                pagination::Key::Vaults,
                items.clone(),
                &ContentPageQuery { limit: 1, cursor },
            )
            .unwrap();
        cursor = next;
    }
    assert_eq!(
        cursors
            .page(
                pagination::Key::Vaults,
                items,
                &ContentPageQuery {
                    limit: 1,
                    cursor: first
                }
            )
            .unwrap_err()
            .code,
        ContentErrorCode::Conflict
    );
}
