use lapis_app_protocol::turns::TurnTarget;
use serde_json::json;

#[test]
fn optional_queue_selection_round_trips_complete_operator_data_without_opaque_inference() {
    let value = json!({
        "id":"opaque", "label":"Display name", "default_mode":"read-only",
        "queue_selection":{
            "recipient":{"id":"writer","label":"Writer","token":"jj-store-agent:777269746572;"},
            "target":{"id":"writer","agent":"codex","thinking":"default", "model":{"provider":"provider","model":"model"}},
            "thinking":"default"
        }
    });
    let target: TurnTarget = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(target).unwrap(), value);
    let absent = json!({"id":"opaque", "label":"Codex", "default_mode":"read-only"});
    let target: TurnTarget = serde_json::from_value(absent.clone()).unwrap();
    assert_eq!(serde_json::to_value(target).unwrap(), absent);
}
