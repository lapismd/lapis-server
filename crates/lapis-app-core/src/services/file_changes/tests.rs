use super::*;
use crate::services::Runtime;
use notify::event::{CreateKind, DataChange};
use std::time::Duration;
use uuid::Uuid;

fn path(frame: &FileChangeSet, expected: &str) -> bool {
    frame.changes.iter().any(|change| match change {
        FileChange::Create { path }
        | FileChange::Modify { path }
        | FileChange::Delete { path }
        | FileChange::Rename { path, .. } => path == expected,
    })
}
async fn receive(
    receiver: &mut broadcast::Receiver<FileChangeSet>,
    expected: &str,
) -> FileChangeSet {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let frame = receiver.recv().await.unwrap();
            if path(&frame, expected) {
                return frame;
            }
        }
    })
    .await
    .expect("native filesystem event must arrive")
}
#[tokio::test]
async fn shared_watch_replays_external_changes_and_retires_with_last_client() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), directory.path()).unwrap();
    let first = vault.attach().unwrap();
    let second = vault.attach().unwrap();
    let mut observation = first.subscribe_to_file_changes(None).unwrap();
    let journal = vault
        .file_changes
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .journal
        .clone();
    let mut peer = second.subscribe_to_file_changes(None).unwrap();
    assert!(Arc::ptr_eq(
        &journal,
        &vault.file_changes.lock().unwrap().as_ref().unwrap().journal
    ));
    let cursor = observation.ready.revision;
    std::fs::write(directory.path().join("note.md"), "external").unwrap();
    receive(&mut observation.changes, "note.md").await;
    receive(&mut peer.changes, "note.md").await;
    let replay = second.subscribe_to_file_changes(Some(cursor)).unwrap();
    assert!(!replay.ready.reset);
    assert!(replay.pending.iter().any(|frame| path(frame, "note.md")));
    first.detach().unwrap();
    assert!(first.subscribe_to_file_changes(None).is_err());
    assert!(vault.file_changes.lock().unwrap().is_some());
    second.detach().unwrap();
    assert!(vault.file_changes.lock().unwrap().is_none());
    drop(journal);
    vault.close().unwrap();
}
#[test]
fn replay_gap_and_future_cursor_require_explicit_reconciliation() {
    let mut journal = Journal {
        revision: 0,
        frames: VecDeque::new(),
        bytes: 0,
        sender: broadcast::channel(HISTORY_FRAMES).0,
    };
    for _ in 0..300 {
        journal.publish(
            vec![FileChange::Modify {
                path: "note.md".into(),
            }],
            false,
        );
    }
    assert!(journal.frames.len() <= HISTORY_FRAMES);
    assert!(journal.bytes <= HISTORY_BYTES);
    assert!(journal.subscribe(Some(0)).ready.reset);
    assert!(journal.subscribe(Some(301)).ready.reset);
    let replay = journal.subscribe(Some(299));
    assert!(!replay.ready.reset);
    assert_eq!(replay.pending.len(), 1);
}
#[test]
fn app_state_caches_and_escaping_paths_never_enter_user_notifications() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join(".lapis")).unwrap();
    let root = VaultRoot::open(directory.path()).unwrap();
    let event = |path: &str| {
        Event::new(EventKind::Create(CreateKind::File)).add_path(root.path().join(path))
    };
    assert!(translate(&root, event(".lapis/plugin-registry-cache/index.json")).is_none());
    assert!(translate(&root, event(".lapis/plugins/tool/main.js")).is_none());
    assert!(translate(&root, event("../outside.md")).is_none());
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), directory.path().join("escape")).unwrap();
    assert!(translate(&root, event("escape/private.md")).is_none());
    assert!(translate(&root, event(".lapis/app.json")).is_some());
    assert!(translate(&root, event("note.md")).is_some());
}
#[test]
fn rename_and_overflow_keep_recovery_explicit() {
    let directory = tempfile::tempdir().unwrap();
    let root = VaultRoot::open(directory.path()).unwrap();
    let event = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
        .add_path(root.path().join("before.md"))
        .add_path(root.path().join("after.md"));
    let changes = translate(&root, event).unwrap().0;
    assert!(
        matches!(&changes[0], FileChange::Rename { path, previous_path } if path == "after.md" && previous_path == "before.md")
    );
    let event = Event::new(EventKind::Any);
    assert!(translate(&root, event).unwrap().1);
    let event = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Any)))
        .add_path(root.path().join("Note.md"));
    assert!(!translate(&root, event).unwrap().1);
}
