use lapis_app_protocol::content::*;
use serde_json::{Value, json};

fn request(command: Value) -> Value {
    json!({"api_version": 1, "request_id": "d2d32935-bb21-4944-926a-28fc8b187fa4", "command": command})
}

fn replace() -> Value {
    json!({"operation": "replace_note", "vault_id": "146098cb-4ccf-49ec-9a58-302e64fc1863", "path": "Inbox/idea.md", "text": "# Idea\n", "expected_version": "sha256:baseline"})
}

#[test]
fn explicit_conditional_mutation_round_trips_without_host_authority() {
    let wire = request(replace());
    let decoded: ContentRequest = serde_json::from_value(wire.clone()).unwrap();
    decoded.validate().unwrap();
    assert_eq!(decoded.command.operation(), ContentOperation::ReplaceNote);
    assert_eq!(serde_json::to_value(decoded).unwrap(), wire);
    let mut unconditional = replace();
    unconditional
        .as_object_mut()
        .unwrap()
        .remove("expected_version");
    assert!(serde_json::from_value::<ContentRequest>(request(unconditional)).is_err());
    for field in ["root", "generation", "client_id", "scope", "force"] {
        let mut command = replace();
        command[field] = json!("caller-authority");
        assert!(
            serde_json::from_value::<ContentRequest>(request(command)).is_err(),
            "{field}"
        );
        let mut envelope = request(replace());
        envelope[field] = json!("caller-authority");
        assert!(
            serde_json::from_value::<ContentRequest>(envelope).is_err(),
            "{field}"
        );
    }
}

#[test]
fn request_validation_rejects_unsafe_paths_and_internal_metadata() {
    for path in [
        "",
        "/note.md",
        "../note.md",
        "a/../note.md",
        "a//note.md",
        "./note.md",
        "a/",
        "C:/note.md",
        "a\\note.md",
        "a\0.md",
        ".lapis/config.json",
        ".LAPIS/config.json",
        ".git/config",
        ".jj/repo",
        "a/.lapis/private",
        ".lapis./config.json",
        ".lapis /config.json",
    ] {
        let mut command = replace();
        command["path"] = json!(path);
        let decoded: ContentRequest = serde_json::from_value(request(command)).unwrap();
        assert_eq!(
            decoded.validate().unwrap_err().code,
            ContentErrorCode::InvalidRequest,
            "{path:?}"
        );
    }
    let mut command = replace();
    command["path"] = json!("研究/日記 🪨.md");
    serde_json::from_value::<ContentRequest>(request(command))
        .unwrap()
        .validate()
        .unwrap();
}

#[test]
fn version_ids_and_payload_budgets_are_validated() {
    let mut wrong_version = request(replace());
    wrong_version["api_version"] = json!(2);
    assert_eq!(
        serde_json::from_value::<ContentRequest>(wrong_version)
            .unwrap()
            .validate()
            .unwrap_err()
            .code,
        ContentErrorCode::ProtocolMismatch
    );
    for field in ["request_id", "vault_id"] {
        let mut wire = request(replace());
        if field == "request_id" {
            wire[field] = json!(uuid::Uuid::nil());
        } else {
            wire["command"][field] = json!(uuid::Uuid::nil());
        }
        assert!(
            serde_json::from_value::<ContentRequest>(wire)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    for value in ["", "*", "bad\nversion", "W/\"weak\""] {
        let mut command = replace();
        command["expected_version"] = json!(value);
        assert!(
            serde_json::from_value::<ContentRequest>(request(command))
                .unwrap()
                .validate()
                .is_err()
        );
    }
    let mut command = replace();
    command["text"] = json!("a".repeat(MAX_CONTENT_TEXT_BYTES));
    serde_json::from_value::<ContentRequest>(request(command.clone()))
        .unwrap()
        .validate()
        .unwrap();
    command["text"] = json!("é".repeat(MAX_CONTENT_TEXT_BYTES / 2 + 1));
    assert_eq!(
        serde_json::from_value::<ContentRequest>(request(command))
            .unwrap()
            .validate()
            .unwrap_err()
            .code,
        ContentErrorCode::TooLarge
    );
}

#[test]
fn pagination_has_explicit_bounds_and_no_native_cursor_authority() {
    for limit in [0, MAX_CONTENT_PAGE_SIZE + 1] {
        let wire = request(json!({"operation":"list_vaults", "page":{"limit":limit}}));
        assert!(
            serde_json::from_value::<ContentRequest>(wire)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    let wire = request(json!({"operation":"list_vaults", "page":{"limit":100,"cursor":"opaque"}}));
    serde_json::from_value::<ContentRequest>(wire)
        .unwrap()
        .validate()
        .unwrap();
    let wire = request(json!({"operation":"list_vaults", "page":{"limit":1,"root":"/tmp"}}));
    assert!(serde_json::from_value::<ContentRequest>(wire).is_err());
}

#[test]
fn path_cursor_and_version_limits_cannot_be_bypassed_by_utf8_or_nested_inputs() {
    let mut command = replace();
    command["path"] = json!("a".repeat(MAX_CONTENT_PATH_BYTES + 1));
    assert!(
        serde_json::from_value::<ContentRequest>(request(command))
            .unwrap()
            .validate()
            .is_err()
    );
    let mut command = replace();
    command["expected_version"] = json!("a".repeat(MAX_CONTENT_VERSION_BYTES + 1));
    assert!(
        serde_json::from_value::<ContentRequest>(request(command))
            .unwrap()
            .validate()
            .is_err()
    );
    for cursor in [
        "".into(),
        "é".into(),
        "two words".into(),
        "a".repeat(MAX_CONTENT_CURSOR_BYTES + 1),
    ] {
        let wire = request(json!({"operation":"list_vaults", "page":{"limit":1,"cursor":cursor}}));
        assert!(
            serde_json::from_value::<ContentRequest>(wire)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    let wire = request(
        json!({"operation":"list_notes", "vault_id":"146098cb-4ccf-49ec-9a58-302e64fc1863", "directory":"../private", "page":{"limit":1}}),
    );
    assert!(
        serde_json::from_value::<ContentRequest>(wire)
            .unwrap()
            .validate()
            .is_err()
    );
    let wire = request(
        json!({"operation":"create_note", "vault_id":"146098cb-4ccf-49ec-9a58-302e64fc1863", "path":"note.md", "text":"new", "overwrite":true}),
    );
    assert!(serde_json::from_value::<ContentRequest>(wire).is_err());
}

#[test]
fn catalog_matches_every_command_and_never_advertises_runtime_support() {
    let commands = [
        json!({"operation":"capabilities"}),
        json!({"operation":"list_vaults","page":{"limit":10}}),
        json!({"operation":"list_notes","vault_id":"146098cb-4ccf-49ec-9a58-302e64fc1863","page":{"limit":10}}),
        json!({"operation":"read_note","vault_id":"146098cb-4ccf-49ec-9a58-302e64fc1863","path":"note.md"}),
        json!({"operation":"create_note","vault_id":"146098cb-4ccf-49ec-9a58-302e64fc1863","path":"note.md","text":"new"}),
        replace(),
    ];
    assert_eq!(CONTENT_OPERATIONS.len(), commands.len());
    for (descriptor, command) in CONTENT_OPERATIONS.iter().zip(commands) {
        let wire_name = command["operation"].clone();
        let decoded: ContentRequest = serde_json::from_value(request(command)).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded.command.operation(), descriptor.operation);
        assert_eq!(
            serde_json::to_value(descriptor.operation).unwrap(),
            wire_name
        );
        assert_eq!(
            descriptor.scope,
            if descriptor.effect == ContentEffect::Write {
                ContentScope::Write
            } else {
                ContentScope::Read
            }
        );
    }
    let capabilities = ContentCapabilities {
        api_version: CONTENT_API_MAJOR,
        operations: vec![],
    };
    assert!(
        serde_json::to_value(capabilities).unwrap()["operations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn unknown_effects_and_domain_errors_are_not_reported_as_rollback() {
    let wire = json!({"status":"error", "error":{"code":"OUTCOME_UNKNOWN", "message":"Inspect the original operation", "effect":"unknown"}});
    let outcome: ContentOutcome = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(outcome).unwrap(), wire);
    let wire = json!({"status":"error", "error":{"code":"BUSY", "message":"Editing intent active", "effect":"not_started"}});
    assert!(serde_json::from_value::<ContentOutcome>(wire).is_ok());
}

#[test]
fn schema_and_typescript_keep_external_contract_separate_from_browser_protocol() {
    let schema = schemars::schema_for!(ContentRequest).to_value();
    assert_eq!(schema["additionalProperties"], false);
    let commands = schema["$defs"]["ContentCommand"]["oneOf"]
        .as_array()
        .unwrap();
    assert_eq!(commands.len(), CONTENT_OPERATIONS.len());
    for command in commands {
        assert_eq!(command["additionalProperties"], false);
    }
    use ts_rs::TS;
    let ts = ContentCommand::decl(&ts_rs::Config::default());
    assert!(ts.contains("expected_version: string"));
    assert!(!ts.contains("generation"));
    assert_eq!(lapis_app_protocol::PROTOCOL_MAJOR, 3);
    assert_eq!(CONTENT_API_MAJOR, 1);
}
