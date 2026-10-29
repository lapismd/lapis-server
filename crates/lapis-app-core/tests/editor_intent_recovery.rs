use lapis_app_core::services::{Client, Runtime, VaultService};
use lapis_app_protocol::ErrorCode;
use serde_json::{Value, json};
use std::{fs, sync::Arc};
use uuid::Uuid;

fn seeded() -> (tempfile::TempDir, Runtime, Arc<VaultService>) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("vault");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("note.md"), "saved bytes").unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), &path).unwrap();
    (root, runtime, vault)
}

fn call(client: &Client, command: Value) -> Result<Value, ErrorCode> {
    let command = serde_json::from_value(command)
        .expect("The protocol must admit captured-state editor recovery");
    client
        .editing_intents(command)
        .map(|value| serde_json::to_value(value).unwrap())
        .map_err(|error| error.code())
}

fn acquire(client: &Client, id: Uuid) {
    call(
        client,
        json!({"kind":"acquire","editor_id":id,"path":"note.md"}),
    )
    .unwrap();
}

fn action(kind: &str, id: Uuid, generation: u32, revision: u32) -> Value {
    json!({"kind":kind,"editor_id":id,"generation":generation,"revision":revision})
}

fn saved(root: &tempfile::TempDir) {
    assert_eq!(
        fs::read_to_string(root.path().join("vault/note.md")).unwrap(),
        "saved bytes"
    );
}

#[test]
fn recovery_preserves_clean_and_dirty_blockers_without_touching_saved_bytes() {
    for dirty in [false, true] {
        let (root, _runtime, vault) = seeded();
        let owner = vault.attach().unwrap();
        let peer = vault.attach().unwrap();
        let id = Uuid::new_v4();
        acquire(&owner, id);
        let revision = if dirty { 2 } else { 0 };
        if dirty {
            call(&owner, action("changed", id, 1, revision)).unwrap();
        }
        owner.detach().unwrap();
        let recovered = call(&peer, action("recover", id, 1, revision)).unwrap();
        assert_eq!(recovered["state"]["generation"], 2);
        assert_eq!(recovered["state"]["revision"], revision);
        assert_eq!(recovered["state"]["dirty"], dirty);
        assert_eq!(recovered["state"]["active"], true);
        assert_eq!(recovered["state"]["detached"], false);
        assert_eq!(vault.close().unwrap_err().code(), ErrorCode::Busy);
        assert_eq!(
            call(&owner, action("discard", id, 1, revision)),
            Err(ErrorCode::PermissionDenied)
        );
        saved(&root);
        assert_eq!(
            call(&peer, action("discard", id, 2, revision)).unwrap(),
            json!({"kind":"retired"})
        );
        saved(&root);
        vault.close().unwrap();
    }
}

#[test]
fn stale_recovery_and_discard_cannot_clear_newer_revision_or_generation() {
    let (root, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    let peer = vault.attach().unwrap();
    let id = Uuid::new_v4();
    acquire(&owner, id);
    call(&owner, action("changed", id, 1, 1)).unwrap();
    let stale_discard = action("discard", id, 1, 1);
    call(&owner, action("changed", id, 1, 2)).unwrap();
    assert_eq!(
        call(&owner, stale_discard.clone()),
        Err(ErrorCode::Conflict)
    );
    owner.detach().unwrap();
    assert_eq!(
        call(&peer, action("recover", id, 1, 1)),
        Err(ErrorCode::Conflict)
    );
    call(&peer, action("recover", id, 1, 2)).unwrap();
    assert_eq!(call(&peer, stale_discard), Err(ErrorCode::Conflict));
    let listed = call(&peer, json!({"kind":"list"})).unwrap();
    assert_eq!(listed["states"][0]["generation"], 2);
    assert_eq!(listed["states"][0]["revision"], 2);
    assert_eq!(listed["states"][0]["dirty"], true);
    assert_eq!(vault.close().unwrap_err().code(), ErrorCode::Busy);
    saved(&root);
    call(&peer, action("discard", id, 2, 2)).unwrap();
    vault.close().unwrap();
}

#[test]
fn live_foreign_owners_and_retired_clients_keep_recovery_fenced() {
    let (root, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    let peer = vault.attach().unwrap();
    let id = Uuid::new_v4();
    acquire(&owner, id);
    assert_eq!(
        call(&peer, action("recover", id, 1, 0)),
        Err(ErrorCode::Busy)
    );
    assert_eq!(
        call(&peer, action("discard", id, 1, 0)),
        Err(ErrorCode::PermissionDenied)
    );
    peer.detach().unwrap();
    owner.detach().unwrap();
    assert_eq!(
        call(&peer, action("discard", id, 1, 0)),
        Err(ErrorCode::PermissionDenied)
    );
    let fresh = vault.attach().unwrap();
    assert_eq!(
        call(&fresh, json!({"kind":"list"})).unwrap()["states"][0]["detached"],
        true
    );
    saved(&root);
    call(&fresh, action("discard", id, 1, 0)).unwrap();
    vault.close().unwrap();
}

#[test]
fn explicit_detached_discard_is_terminal_and_reconciles_a_lost_acknowledgement() {
    let (root, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    let peer = vault.attach().unwrap();
    let id = Uuid::new_v4();
    acquire(&owner, id);
    owner.detach().unwrap();
    let discard = action("discard", id, 1, 0);
    call(&peer, discard.clone()).unwrap();
    assert_eq!(call(&peer, discard).unwrap(), json!({"kind":"retired"}));
    assert_eq!(
        call(&peer, json!({"kind":"inspect","editor_id":id})).unwrap(),
        json!({"kind":"retired"})
    );
    assert_eq!(
        call(
            &peer,
            json!({"kind":"acquire","editor_id":id,"path":"note.md"})
        ),
        Err(ErrorCode::Closed)
    );
    saved(&root);
    vault.close().unwrap();
}

#[test]
fn replaced_physical_root_cannot_discard_a_retained_blocker() {
    let (root, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    let peer = vault.attach().unwrap();
    let id = Uuid::new_v4();
    acquire(&owner, id);
    owner.detach().unwrap();
    let original = root.path().join("vault");
    let retained = root.path().join("retained");
    fs::rename(&original, &retained).unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("note.md"), "replacement").unwrap();
    assert!(call(&peer, action("discard", id, 1, 0)).is_err());
    fs::remove_dir_all(&original).unwrap();
    fs::rename(&retained, &original).unwrap();
    assert_eq!(
        call(&peer, json!({"kind":"list"})).unwrap()["states"][0]["detached"],
        true
    );
    saved(&root);
    call(&peer, action("discard", id, 1, 0)).unwrap();
    vault.close().unwrap();
}

#[test]
fn legacy_id_only_discard_cannot_clear_a_managed_intent() {
    let (root, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    let id = Uuid::new_v4();
    acquire(&owner, id);
    assert_eq!(
        owner.discard_editor(id).unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        call(&owner, json!({"kind":"inspect","editor_id":id})).unwrap()["state"]["active"],
        true
    );
    let legacy = owner.open_editor("note.md").unwrap();
    owner.mark_dirty(legacy, 1).unwrap();
    owner.discard_editor(legacy).unwrap();
    call(&owner, action("discard", id, 1, 0)).unwrap();
    saved(&root);
    vault.close().unwrap();
}

#[test]
fn legacy_id_only_recovery_cannot_rebind_a_managed_intent() {
    let (root, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    let peer = vault.attach().unwrap();
    let id = Uuid::new_v4();
    acquire(&owner, id);
    let legacy = owner.open_editor("note.md").unwrap();
    owner.mark_dirty(legacy, 1).unwrap();
    owner.detach().unwrap();
    assert_eq!(
        peer.recover_editor(id).unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        call(&peer, json!({"kind":"list"})).unwrap()["states"][0]["detached"],
        true
    );
    peer.recover_editor(legacy).unwrap();
    peer.discard_editor(legacy).unwrap();
    call(&peer, action("discard", id, 1, 0)).unwrap();
    saved(&root);
    vault.close().unwrap();
}
