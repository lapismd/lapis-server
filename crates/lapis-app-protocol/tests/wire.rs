use lapis_app_protocol::*;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

fn fixture() -> Value {
    json!({"protocol_version": 3, "request_id": "d2d32935-bb21-4944-926a-28fc8b187fa4", "vault_id": "146098cb-4ccf-49ec-9a58-302e64fc1863", "generation": "6d4bd7c0-14a6-4b2a-bc82-53723606ff5a", "command": {"kind": "write_text", "path": "note.md", "text": "hello", "expected": {"kind": "missing"}}})
}
#[test]
fn wire_fixture_round_trips_without_a_transport() {
    let value = fixture();
    let request: Request = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(request).unwrap(), value);
}
#[test]
fn unknown_fields_commands_and_invalid_ids_fail_closed() {
    let mut value = fixture();
    value["root"] = json!("/tmp/another-vault");
    assert!(serde_json::from_value::<Request>(value).is_err());
    let mut value = fixture();
    value["command"]["root"] = json!("/tmp/another-vault");
    assert!(serde_json::from_value::<Request>(value).is_err());
    let mut value = fixture();
    value["command"]["kind"] = json!("unknown");
    assert!(serde_json::from_value::<Request>(value).is_err());
    let mut value = fixture();
    value["vault_id"] = json!("not-a-uuid");
    assert!(serde_json::from_value::<Request>(value).is_err());
    let mut value = fixture();
    value["command"] = json!({"kind": "status", "root": "/tmp"});
    assert!(serde_json::from_value::<Request>(value).is_err());
    let mut value = fixture();
    value["command"]["expected"]["version"] = json!("unrequested");
    assert!(serde_json::from_value::<Request>(value).is_err());
}
#[test]
fn schema_keeps_required_scope_and_strict_command_shapes() {
    let schema = serde_json::to_value(schemars::schema_for!(Request)).unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["vault_id"]["format"], "uuid");
    let required = schema["required"].as_array().unwrap();
    for field in [
        "protocol_version",
        "request_id",
        "vault_id",
        "generation",
        "command",
    ] {
        assert!(required.contains(&json!(field)));
    }
    let commands = schema["$defs"]["VaultCommand"]["oneOf"].as_array().unwrap();
    for command in commands {
        assert_eq!(command["additionalProperties"], false);
    }
}
#[test]
fn explicit_typescript_annotations_match_serde_discriminants() {
    let cfg = Config::default();
    for (command, label) in [
        (VaultCommand::Status {}, "status"),
        (VaultCommand::EditorClaims {}, "editor_claims"),
    ] {
        assert_eq!(serde_json::to_value(command).unwrap()["kind"], label);
        assert!(VaultCommand::decl(&cfg).contains(&format!("\"{label}\"")));
    }
    assert!(ErrorCode::decl(&cfg).contains("PROTOCOL_MISMATCH"));
    assert_eq!(
        serde_json::to_value(ErrorCode::ProtocolMismatch).unwrap(),
        "PROTOCOL_MISMATCH"
    );
    assert!(Outcome::decl(&cfg).contains("status"));
    assert!(ExpectedVersion::decl(&cfg).contains("missing"));
}

#[test]
fn existing_file_frames_keep_camel_case_defaults_and_native_action_exclusion() {
    use lapis_app_protocol::files::{FileCommand, FileValue};
    let command: FileCommand = serde_json::from_value(
        json!({"operation": "rename", "path": "old.md", "newPath": "new.md"}),
    )
    .unwrap();
    assert!(matches!(command, FileCommand::Rename { .. }));
    let command: FileCommand = serde_json::from_value(
        json!({"operation": "write-text", "path": "note.md", "data": "new"}),
    )
    .unwrap();
    let FileCommand::WriteText { options, .. } = command else {
        panic!()
    };
    assert_eq!(options.mtime, None);
    assert!(
        serde_json::from_value::<FileCommand>(json!({"operation": "open-path", "path": "note.md"}))
            .is_err()
    );
    assert!(
        serde_json::from_value::<FileCommand>(
            json!({"operation": "read-text", "path": "note.md", "root": "/tmp"})
        )
        .is_err()
    );
    let encoded = serde_json::to_value(FileValue::Done {}).unwrap();
    assert_eq!(encoded, json!({"kind": "done"}));
    assert!(
        serde_json::from_value::<FileValue>(json!({"kind": "done", "unexpected": true})).is_err()
    );
    let cfg = Config::default();
    assert!(FileCommand::decl(&cfg).contains("newPath"));
    assert!(!FileCommand::decl(&cfg).contains("new_path"));
}
