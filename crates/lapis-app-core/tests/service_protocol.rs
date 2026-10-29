use lapis_app_core::services::{Client, Runtime};
use lapis_app_protocol::{
    ErrorCode, ExpectedVersion, Outcome, PROTOCOL_MAJOR, Request, VaultCommand, VaultValue,
};
use uuid::Uuid;

fn request(client: &Client, command: VaultCommand) -> Request {
    let descriptor = client.descriptor();
    Request {
        protocol_version: PROTOCOL_MAJOR,
        request_id: Uuid::new_v4(),
        vault_id: descriptor.vault_id,
        generation: descriptor.generation,
        command,
    }
}
fn denied(client: &Client, request: Request, expected: ErrorCode) {
    let id = request.request_id;
    let reply = client.execute(request);
    assert_eq!(reply.request_id, id);
    assert_eq!(reply.protocol_version, PROTOCOL_MAJOR);
    let Outcome::Error { error } = reply.outcome else {
        panic!("request was accepted")
    };
    assert_eq!(error.code, expected);
}
fn write() -> VaultCommand {
    VaultCommand::WriteText {
        path: "note.md".into(),
        text: "hello".into(),
        expected: ExpectedVersion::Missing {},
    }
}
#[test]
fn invalid_version_identity_vault_and_generation_cannot_mutate_files() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), dir.path()).unwrap();
    let client = vault.attach().unwrap();
    let mut invalid = request(&client, write());
    invalid.protocol_version -= 1;
    denied(&client, invalid, ErrorCode::ProtocolMismatch);
    let mut invalid = request(&client, write());
    invalid.request_id = Uuid::nil();
    denied(&client, invalid, ErrorCode::InvalidRequest);
    let mut invalid = request(&client, write());
    invalid.vault_id = Uuid::new_v4();
    denied(&client, invalid, ErrorCode::PermissionDenied);
    let mut invalid = request(&client, write());
    invalid.generation = Uuid::new_v4();
    denied(&client, invalid, ErrorCode::PermissionDenied);
    assert!(!dir.path().join("note.md").exists());
}
#[test]
fn typed_round_trip_preserves_unicode_and_rejects_stale_publication() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), dir.path()).unwrap();
    let client = vault.attach().unwrap();
    let command = VaultCommand::WriteText {
        path: "é.md".into(),
        text: "# 💎\n".into(),
        expected: ExpectedVersion::Missing {},
    };
    let encoded = serde_json::to_vec(&request(&client, command)).unwrap();
    let reply = client.execute(serde_json::from_slice(&encoded).unwrap());
    assert!(matches!(
        reply.outcome,
        Outcome::Ok {
            value: VaultValue::Written { .. }
        }
    ));
    let reply = client.execute(request(
        &client,
        VaultCommand::ReadText {
            path: "é.md".into(),
        },
    ));
    let Outcome::Ok {
        value: VaultValue::Text { snapshot },
    } = reply.outcome
    else {
        panic!()
    };
    assert_eq!(snapshot.text, "# 💎\n");
    assert_eq!(snapshot.version.len(), 64);
    denied(
        &client,
        request(
            &client,
            VaultCommand::WriteText {
                path: "é.md".into(),
                text: "lost".into(),
                expected: ExpectedVersion::Missing {},
            },
        ),
        ErrorCode::Conflict,
    );
}
#[test]
fn editor_claims_and_resolution_use_the_same_command_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), dir.path()).unwrap();
    let client = vault.attach().unwrap();
    let reply = client.execute(request(
        &client,
        VaultCommand::OpenEditor {
            path: "new.md".into(),
        },
    ));
    let Outcome::Ok {
        value: VaultValue::Editor { editor_id },
    } = reply.outcome
    else {
        panic!()
    };
    assert!(matches!(
        client
            .execute(request(
                &client,
                VaultCommand::MarkDirty {
                    editor_id,
                    revision: 1
                }
            ))
            .outcome,
        Outcome::Ok { .. }
    ));
    drop(client);
    let client = vault.attach().unwrap();
    let reply = client.execute(request(&client, VaultCommand::EditorClaims {}));
    let Outcome::Ok {
        value: VaultValue::EditorClaims { claims },
    } = reply.outcome
    else {
        panic!()
    };
    assert_eq!(claims.len(), 1);
    assert!(claims[0].detached);
    assert!(matches!(
        client
            .execute(request(&client, VaultCommand::RecoverEditor { editor_id }))
            .outcome,
        Outcome::Ok { .. }
    ));
    assert!(matches!(
        client
            .execute(request(&client, VaultCommand::DiscardEditor { editor_id }))
            .outcome,
        Outcome::Ok { .. }
    ));
    vault.close().unwrap();
    denied(&client, request(&client, write()), ErrorCode::Closed);
}
#[test]
fn previous_generation_cannot_write_after_a_vault_is_reopened() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let vault = runtime.admit(id, dir.path()).unwrap();
    let client = vault.attach().unwrap();
    let old = request(&client, write());
    runtime.close_vault(id).unwrap();
    let reopened = runtime.admit(id, dir.path()).unwrap();
    let fresh = reopened.attach().unwrap();
    denied(&fresh, old, ErrorCode::PermissionDenied);
    assert!(!dir.path().join("note.md").exists());
}
#[test]
fn dirty_editor_guards_cover_hard_link_aliases() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("note.md"), "original").unwrap();
    std::fs::hard_link(dir.path().join("note.md"), dir.path().join("alias.md")).unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), dir.path()).unwrap();
    let owner = vault.attach().unwrap();
    let editor = owner.open_editor("note.md").unwrap();
    owner.mark_dirty(editor, 1).unwrap();
    let other = vault.attach().unwrap();
    let version = other.read_text("alias.md").unwrap().version;
    assert_eq!(
        other
            .write_text("alias.md", "unsafe", &ExpectedVersion::Version { version })
            .unwrap_err()
            .code(),
        ErrorCode::Busy
    );
}
