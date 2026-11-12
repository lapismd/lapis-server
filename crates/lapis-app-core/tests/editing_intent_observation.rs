use lapis_app_core::services::{Client, Runtime};
use lapis_app_protocol::{ErrorCode, editing_intents::EditingIntentCommand as Command};
use uuid::Uuid;

fn acquire(client: &Client, editor_id: Uuid) {
    client
        .editing_intents(Command::Acquire {
            editor_id,
            path: "Note.md".into(),
        })
        .unwrap();
}

#[test]
fn invalidations_cover_changes_detach_recovery_and_discard_without_read_loops() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Note.md"), "saved bytes").unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let owner = vault.attach().unwrap();
    let peer = vault.attach().unwrap();
    let mut observation = peer.subscribe_to_editing_intents().unwrap();
    assert_eq!(*observation.borrow_and_update(), 0);
    let id = Uuid::new_v4();
    acquire(&owner, id);
    assert_eq!(*observation.borrow_and_update(), 1);
    acquire(&owner, id);
    peer.editing_intents(Command::List {}).unwrap();
    assert!(
        !observation.has_changed().unwrap(),
        "reads and exact retries never invalidate"
    );
    owner
        .editing_intents(Command::Changed {
            editor_id: id,
            generation: 1,
            revision: 2,
        })
        .unwrap();
    assert_eq!(*observation.borrow_and_update(), 2);
    owner.detach().unwrap();
    assert_eq!(*observation.borrow_and_update(), 3);
    owner.detach().unwrap();
    assert!(!observation.has_changed().unwrap());
    assert_eq!(vault.close().unwrap_err().code(), ErrorCode::Busy);
    peer.editing_intents(Command::Recover {
        editor_id: id,
        generation: 1,
        revision: 2,
    })
    .unwrap();
    assert_eq!(*observation.borrow_and_update(), 4);
    peer.editing_intents(Command::Discard {
        editor_id: id,
        generation: 2,
        revision: 2,
    })
    .unwrap();
    assert_eq!(*observation.borrow_and_update(), 5);
    assert_eq!(
        std::fs::read_to_string(root.path().join("Note.md")).unwrap(),
        "saved bytes"
    );
    vault.close().unwrap();
    assert!(peer.validate_editing_intent_observation().is_err());
}

#[test]
fn slow_observers_coalesce_changes_and_cannot_cross_vault_or_retirement_scope() {
    let root = tempfile::tempdir().unwrap();
    let other_root = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let other_vault = runtime.admit(Uuid::new_v4(), other_root.path()).unwrap();
    let owner = vault.attach().unwrap();
    let other = other_vault.attach().unwrap();
    let mut observation = owner.subscribe_to_editing_intents().unwrap();
    acquire(&other, Uuid::new_v4());
    assert!(!observation.has_changed().unwrap());
    let id = Uuid::new_v4();
    acquire(&owner, id);
    for revision in 1..=1000 {
        owner
            .editing_intents(Command::Changed {
                editor_id: id,
                generation: 1,
                revision,
            })
            .unwrap();
    }
    assert_eq!(*observation.borrow_and_update(), 1001);
    assert!(!observation.has_changed().unwrap());
    owner.detach().unwrap();
    assert!(owner.subscribe_to_editing_intents().is_err());
    assert!(owner.validate_editing_intent_observation().is_err());
}

#[test]
fn observation_rejects_a_replaced_physical_root() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("vault");
    std::fs::create_dir(&root).unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), &root).unwrap();
    let owner = vault.attach().unwrap();
    owner.subscribe_to_editing_intents().unwrap();
    std::fs::rename(&root, directory.path().join("original")).unwrap();
    std::fs::create_dir(&root).unwrap();
    assert!(owner.validate_editing_intent_observation().is_err());
    assert!(owner.subscribe_to_editing_intents().is_err());
}
