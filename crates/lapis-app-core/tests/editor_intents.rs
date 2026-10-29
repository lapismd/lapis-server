use lapis_app_core::services::{Client, Runtime, VaultService};
use lapis_app_protocol::{ErrorCode, ExpectedVersion, Outcome, PROTOCOL_MAJOR, Request};
use serde_json::{Value, json};
use std::{fs, sync::Arc};
use uuid::Uuid;

fn seeded() -> (tempfile::TempDir, Runtime, Arc<VaultService>) {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("note.md"), "original").unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    (root, runtime, vault)
}

fn call(client: &Client, command: Value) -> Result<Value, ErrorCode> {
    let descriptor = client.descriptor();
    let command = serde_json::from_value(json!({
        "kind": "editing_intents", "command": command
    }))
    .expect("The shared protocol must admit native editor intents");
    let reply = client.execute(Request {
        protocol_version: PROTOCOL_MAJOR,
        request_id: Uuid::new_v4(),
        vault_id: descriptor.vault_id,
        generation: descriptor.generation,
        command,
    });
    match reply.outcome {
        Outcome::Ok { value } => Ok(serde_json::to_value(value).unwrap()["value"].clone()),
        Outcome::Error { error } => Err(error.code),
    }
}

fn acquire(client: &Client, id: Uuid, path: &str) -> Value {
    call(
        client,
        json!({"kind":"acquire", "editor_id":id, "path":path}),
    )
    .unwrap()
}

fn expected(client: &Client, path: &str) -> Value {
    json!({"kind":"version", "version":client.read_text(path).unwrap().version})
}

fn yield_intent(
    client: &Client,
    id: Uuid,
    generation: u32,
    revision: u32,
    expected: Value,
) -> Value {
    call(
        client,
        json!({"kind":"yield", "editor_id":id, "generation":generation,
        "revision":revision, "expected":expected}),
    )
    .unwrap()
}

#[test]
fn clean_editable_intent_fences_foreign_writers_until_guarded_yield() {
    let (_root, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    let other = vault.attach().unwrap();
    let id = Uuid::new_v4();
    let acquired = acquire(&owner, id, "note.md");
    assert_eq!(acquired["state"]["active"], true);
    assert_eq!(acquired["state"]["dirty"], false);
    assert_eq!(vault.status().unwrap().dirty_editors, 0);
    let original = owner.read_text("note.md").unwrap();
    let version = ExpectedVersion::Version {
        version: original.version,
    };
    assert_eq!(
        other
            .write_text("note.md", "unsafe", &version)
            .unwrap_err()
            .code(),
        ErrorCode::Busy
    );
    assert_eq!(
        vault
            .begin_work()
            .unwrap()
            .write_text("note.md", "unsafe", &version)
            .unwrap_err()
            .code(),
        ErrorCode::Busy
    );
    owner.write_text("note.md", "owned save", &version).unwrap();
    yield_intent(&owner, id, 1, 0, expected(&owner, "note.md"));
    let current = owner.read_text("note.md").unwrap();
    other
        .write_text(
            "note.md",
            "guarded mutation",
            &ExpectedVersion::Version {
                version: current.version,
            },
        )
        .unwrap();
    call(
        &owner,
        json!({"kind":"resume", "editor_id":id, "generation":2}),
    )
    .unwrap();
    assert_eq!(vault.close().unwrap_err().code(), ErrorCode::Busy);
    assert_eq!(other.read_text("note.md").unwrap().text, "guarded mutation");
}

#[test]
fn stale_saved_generation_cannot_clear_newer_dirty_or_intent_state() {
    let (_root, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    let id = Uuid::new_v4();
    acquire(&owner, id, "note.md");
    for revision in [1, 2] {
        call(
            &owner,
            json!({"kind":"changed", "editor_id":id, "generation":1, "revision":revision}),
        )
        .unwrap();
    }
    assert_eq!(
        call(
            &owner,
            json!({"kind":"yield", "editor_id":id, "generation":1,
        "revision":1, "expected":expected(&owner,"note.md")})
        ),
        Err(ErrorCode::Conflict)
    );
    assert_eq!(vault.status().unwrap().dirty_editors, 1);
    yield_intent(&owner, id, 1, 2, expected(&owner, "note.md"));
    call(
        &owner,
        json!({"kind":"resume", "editor_id":id, "generation":2}),
    )
    .unwrap();
    assert_eq!(
        call(
            &owner,
            json!({"kind":"yield", "editor_id":id, "generation":1,
        "revision":2, "expected":expected(&owner,"note.md")})
        ),
        Err(ErrorCode::Conflict)
    );
    assert_eq!(
        call(&owner, json!({"kind":"inspect", "editor_id":id})).unwrap()["state"]["active"],
        true
    );
}

#[test]
fn expected_absence_and_existing_file_versions_are_distinct() {
    let (root, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    let id = Uuid::new_v4();
    acquire(&owner, id, "new.md");
    yield_intent(&owner, id, 1, 0, json!({"kind":"missing"}));
    call(
        &owner,
        json!({"kind":"resume", "editor_id":id, "generation":2}),
    )
    .unwrap();
    fs::write(root.path().join("new.md"), "external").unwrap();
    assert_eq!(
        call(
            &owner,
            json!({"kind":"yield", "editor_id":id, "generation":2,
        "revision":0, "expected":{"kind":"missing"}})
        ),
        Err(ErrorCode::Conflict)
    );
    assert_eq!(
        call(&owner, json!({"kind":"inspect", "editor_id":id})).unwrap()["state"]["active"],
        true
    );
    assert_eq!(
        fs::read_to_string(root.path().join("new.md")).unwrap(),
        "external"
    );
}

#[test]
fn client_retirement_retains_clean_intent_until_explicit_recovery() {
    let (_root, _runtime, vault) = seeded();
    let owner = Arc::new(vault.attach().unwrap());
    let id = Uuid::new_v4();
    acquire(&owner, id, "note.md");
    owner.detach().unwrap();
    assert_eq!(
        owner.read_text("note.md").unwrap_err().code(),
        ErrorCode::PermissionDenied
    );
    let reconnect = vault.attach().unwrap();
    let list = call(&reconnect, json!({"kind":"list"})).unwrap();
    assert_eq!(list["states"][0]["dirty"], false);
    assert_eq!(list["states"][0]["active"], true);
    assert_eq!(list["states"][0]["detached"], true);
    assert_eq!(vault.close().unwrap_err().code(), ErrorCode::Busy);
    call(
        &reconnect,
        json!({"kind":"recover", "editor_id":id, "generation":1, "revision":0}),
    )
    .unwrap();
    yield_intent(&reconnect, id, 2, 0, expected(&reconnect, "note.md"));
    call(
        &reconnect,
        json!({"kind":"retire", "editor_id":id, "generation":2}),
    )
    .unwrap();
    assert!(
        call(&reconnect, json!({"kind":"list"})).unwrap()["states"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn identities_and_generations_are_idempotent_without_replaying_content() {
    let (_root, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    let id = Uuid::new_v4();
    assert_eq!(
        acquire(&owner, id, "note.md"),
        acquire(&owner, id, "note.md")
    );
    for _ in 0..2 {
        call(
            &owner,
            json!({"kind":"changed", "editor_id":id, "generation":1, "revision":1}),
        )
        .unwrap();
    }
    yield_intent(&owner, id, 1, 1, expected(&owner, "note.md"));
    for _ in 0..2 {
        call(
            &owner,
            json!({"kind":"resume", "editor_id":id, "generation":2}),
        )
        .unwrap();
    }
    let inspected = call(&owner, json!({"kind":"inspect", "editor_id":id})).unwrap();
    assert_eq!(inspected["state"]["generation"], 2);
    yield_intent(&owner, id, 2, 1, expected(&owner, "note.md"));
    for _ in 0..2 {
        call(
            &owner,
            json!({"kind":"retire", "editor_id":id, "generation":2}),
        )
        .unwrap();
    }
    assert_eq!(
        call(&owner, json!({"kind":"inspect", "editor_id":id})).unwrap()["kind"],
        "retired"
    );
    assert_eq!(
        call(
            &owner,
            json!({"kind":"acquire", "editor_id":id, "path":"note.md"})
        ),
        Err(ErrorCode::Closed)
    );
    assert_eq!(owner.read_text("note.md").unwrap().text, "original");
}

#[test]
fn foreign_identity_and_path_cannot_replace_an_admitted_intent() {
    let (_root, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    let other = vault.attach().unwrap();
    let id = Uuid::new_v4();
    acquire(&owner, id, "note.md");
    for (client, path) in [(&other, "note.md"), (&owner, "different.md")] {
        assert_eq!(
            call(
                client,
                json!({"kind":"acquire", "editor_id":id, "path":path})
            ),
            Err(ErrorCode::PermissionDenied)
        );
    }
    assert_eq!(
        call(&other, json!({"kind":"inspect", "editor_id":id})),
        Err(ErrorCode::PermissionDenied)
    );
    assert_eq!(
        call(
            &owner,
            json!({"kind":"acquire", "editor_id":Uuid::nil(), "path":"note.md"})
        ),
        Err(ErrorCode::InvalidRequest)
    );
}

#[test]
fn active_intents_retain_existing_physical_alias_and_descendant_fences() {
    let (root, _runtime, vault) = seeded();
    fs::hard_link(root.path().join("note.md"), root.path().join("alias.md")).unwrap();
    let owner = vault.attach().unwrap();
    let other = vault.attach().unwrap();
    acquire(&owner, Uuid::new_v4(), "note.md");
    let version = ExpectedVersion::Version {
        version: other.read_text("alias.md").unwrap().version,
    };
    assert_eq!(
        other
            .write_text("alias.md", "unsafe", &version)
            .unwrap_err()
            .code(),
        ErrorCode::Busy
    );
    fs::create_dir(root.path().join("folder")).unwrap();
    fs::write(root.path().join("folder/child.md"), "child").unwrap();
    acquire(&owner, Uuid::new_v4(), "folder/child.md");
    let result = other.files(lapis_app_protocol::files::FileCommand::Rename {
        path: "folder".into(),
        new_path: "moved".into(),
    });
    assert_eq!(result.unwrap_err().code(), ErrorCode::Busy);
    assert!(root.path().join("folder/child.md").exists());
}

#[test]
fn clean_yield_does_not_allow_dirty_or_stale_retirement() {
    let (_root, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    let id = Uuid::new_v4();
    acquire(&owner, id, "note.md");
    assert_eq!(
        call(
            &owner,
            json!({"kind":"retire", "editor_id":id, "generation":1})
        ),
        Err(ErrorCode::Busy)
    );
    yield_intent(&owner, id, 1, 0, expected(&owner, "note.md"));
    call(
        &owner,
        json!({"kind":"resume", "editor_id":id, "generation":2}),
    )
    .unwrap();
    assert_eq!(
        call(
            &owner,
            json!({"kind":"retire", "editor_id":id, "generation":1})
        ),
        Err(ErrorCode::Conflict)
    );
    assert_eq!(
        call(&owner, json!({"kind":"inspect", "editor_id":id})).unwrap()["state"]["active"],
        true
    );
}

#[test]
fn active_intent_admission_stays_bounded_without_converting_clean_content_to_dirty() {
    let (_root, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    for _ in 0..4096 {
        acquire(&owner, Uuid::new_v4(), "note.md");
    }
    assert_eq!(
        call(
            &owner,
            json!({"kind":"acquire", "editor_id":Uuid::new_v4(), "path":"note.md"})
        ),
        Err(ErrorCode::Busy)
    );
    assert_eq!(vault.status().unwrap().dirty_editors, 0);
    assert_eq!(vault.close().unwrap_err().code(), ErrorCode::Busy);
}

#[test]
fn explicit_discard_does_not_let_delayed_acquire_restore_an_old_intent() {
    let (_root, _runtime, vault) = seeded();
    let owner = vault.attach().unwrap();
    let id = Uuid::new_v4();
    acquire(&owner, id, "note.md");
    call(
        &owner,
        json!({"kind":"discard", "editor_id":id, "generation":1, "revision":0}),
    )
    .unwrap();
    assert_eq!(
        call(
            &owner,
            json!({"kind":"acquire", "editor_id":id, "path":"note.md"})
        ),
        Err(ErrorCode::Closed)
    );
    assert_eq!(
        call(&owner, json!({"kind":"inspect", "editor_id":id})).unwrap()["kind"],
        "retired"
    );
    vault.close().unwrap();
}
