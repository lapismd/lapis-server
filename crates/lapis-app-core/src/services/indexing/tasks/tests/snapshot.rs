use super::support::*;
use crate::vault::VaultRoot;
use std::sync::{Mutex, atomic::AtomicBool};
use uuid::Uuid;

#[test]
fn binary_attachment_targets_are_unsupported_and_keep_exact_byte_freshness() {
    let root = tempfile::tempdir().unwrap();
    write(
        &root,
        "Parent.md",
        &document("parent", "task-list", "- [Photo](Photo.png)"),
    );
    std::fs::write(root.path().join("Photo.png"), [0xff, 0x80]).unwrap();
    let admitted = VaultRoot::open(root.path()).unwrap();
    let stop = AtomicBool::new(false);
    let files = vec![String::from("Parent.md"), String::from("Photo.png")];
    let snapshot = super::super::snapshot::Snapshot::prepare(&admitted, &files, &stop).unwrap();
    assert_eq!(
        serde_json::to_value(&snapshot.structures["Parent.md"]).unwrap()["diagnostics"][0]["code"],
        "unsupported-structural-target"
    );
    assert!(snapshot.unchanged(&admitted, &files, &stop).unwrap());
    std::fs::write(root.path().join("Photo.png"), [0xfe, 0x81]).unwrap();
    assert!(!snapshot.unchanged(&admitted, &files, &stop).unwrap());
}

#[test]
fn changed_bytes_or_added_targets_invalidate_an_admitted_snapshot() {
    let root = tempfile::tempdir().unwrap();
    write(
        &root,
        "Parent.md",
        &document("parent", "task-list", "- [New](New.md)"),
    );
    let admitted = VaultRoot::open(root.path()).unwrap();
    let stop = AtomicBool::new(false);
    let files = vec![String::from("Parent.md")];
    let snapshot = super::super::snapshot::Snapshot::prepare(&admitted, &files, &stop).unwrap();
    assert!(snapshot.unchanged(&admitted, &files, &stop).unwrap());
    write(&root, "New.md", &document("new", "task", ""));
    assert!(!snapshot.unchanged(&admitted, &files, &stop).unwrap());
    std::fs::remove_file(root.path().join("New.md")).unwrap();
    write(
        &root,
        "Parent.md",
        &document("parent", "task-list", "- [Now](Now.md)"),
    );
    assert!(!snapshot.unchanged(&admitted, &files, &stop).unwrap());
}

#[tokio::test]
async fn a_source_deleted_after_enumeration_is_pending_without_partial_rows() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let database = vault.database().await.unwrap();
    let admitted = VaultRoot::open(root.path()).unwrap();
    let work = vault.begin_work().unwrap();
    let pending = super::super::run(
        &work,
        &database,
        &admitted,
        &[String::from("Gone.md")],
        &AtomicBool::new(false),
        super::super::Selection {
            selected: &AtomicBool::new(true),
            tracked: &Mutex::new(None),
            today: "2026-08-20",
        },
    )
    .await
    .unwrap();
    assert_eq!(pending.pending, 1);
    assert!(pending.tracked.is_none());
    assert_eq!(query(&vault).await.unwrap()["rows"], serde_json::json!([]));
    drop(work);
    close(runtime).await;
}
