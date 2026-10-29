use lapis_app_protocol::native_tasks::*;
use serde_json::json;
use ts_rs::{Config, TS};

#[test]
fn native_tasks_wire_tags_and_typescript_declarations_share_the_same_shape() {
    let request = json!({"kind":"submit","request":{"request_id":"493fb51b-cc8f-4f0c-b79e-11163126af66","path":"Task.md","expected_version":"a".repeat(64),"action":{"type":"complete","at":"2026-10-26T09:00:00Z","localDate":"2026-10-26","trackedValue":null}}});
    let command: NativeTaskCommand = serde_json::from_value(request.clone()).unwrap();
    assert_eq!(serde_json::to_value(command).unwrap(), request);
    let config = Config::default();
    let declaration = NativeTaskCommand::decl(&config);
    assert!(declaration.contains("\"kind\": \"submit\""));
    assert!(declaration.contains("NativeTaskLifecycleRequest"));
    let declaration = NativeTaskLifecycleAction::decl(&config);
    assert!(declaration.contains("\"type\": \"complete\""));
    assert!(declaration.contains("localDate: string"));
    assert!(declaration.contains("trackedValue: number | null"));
    assert!(!declaration.contains("local_date"));
}

#[test]
fn native_tasks_wire_rejects_unknown_fields_at_each_nested_authority_boundary() {
    let configuration = json!({"day_parts":{"afternoonStartsAt":"12:00","eveningStartsAt":"17:00"},"daily":{"folder":"daily","date_format":"yyyy-LL-dd"},"format":{"locale":"en-US","firstDay":7,"minimalDays":4},"provider":{"kind":"default"}});
    for field in [
        None,
        Some("day_parts"),
        Some("daily"),
        Some("format"),
        Some("provider"),
    ] {
        let mut value = configuration.clone();
        // Keep both enum and struct deserialization fail closed.
        if let Some(field) = field {
            value[field]["callback"] = json!("write");
        } else {
            value["root"] = json!("/outside");
        }
        assert!(
            serde_json::from_value::<NativeTaskCommand>(
                json!({"kind":"configure","configuration":value})
            )
            .is_err()
        );
    }
    assert!(
        serde_json::from_value::<NativeTaskCommand>(json!({"kind":"status","vault_id":"foreign"}))
            .is_err()
    );
    assert!(
        serde_json::from_value::<NativeTaskMutation>(json!({"type":"complete","at":"now"}))
            .is_err()
    );
}
