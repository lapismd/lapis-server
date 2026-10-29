use super::{QueueInput, input};
use serde_json::{Value, json};
use uuid::Uuid;

// Frozen Desktop panel/queue-routing.ts uses target.thinking ?? selection.thinking.
// Its dispatch path then applies that exact per-target request over the capture.
#[test]
fn raw_queue_thinking_preserves_nullish_inheritance_and_explicit_defaults() {
    for (target, global, expected) in [
        (Some(Value::Null), Some(json!("high")), Some("high")),
        (None, Some(json!("high")), Some("high")),
        (Some(json!("default")), Some(json!("high")), None),
        (Some(json!("off")), Some(json!("high")), Some("off")),
        (Some(json!("low")), Some(json!("high")), Some("low")),
        (Some(Value::Null), Some(Value::Null), None),
        (None, None, None),
    ] {
        let mut capture = json!({
            "version":1,"id":"capture-thinking","state":"queued",
            "user":{
                "schemaVersion":3,"id":"queued-user-thinking","type":"message","role":"user",
                "text":"Retained thinking choice", "createdAt":"2026-10-27T00:00:00.000Z",
                "recipients":[{"id":"writer","label":"Writer","token":"jj-store-agent:777269746572;"}]
            },
            "selection":{"runtime":"acp","attachments":[],"targets":[{"id":"writer","agent":"codex"}]}
        });
        if let Some(value) = target {
            capture["selection"]["targets"][0]["thinking"] = value;
        }
        if let Some(value) = global {
            capture["selection"]["thinking"] = value;
        }
        crate::turns::queue_capture::validate_message(capture.clone()).unwrap();
        let original = capture.clone();
        let accepted = QueueInput {
            acceptance_id: Uuid::new_v4(),
            conversation_id: Uuid::new_v4(),
            after_request_id: Uuid::new_v4(),
            scope_dir: String::new(),
            target_id: "opaque-runtime".into(),
            capture,
        };
        let native = input(&accepted, Uuid::new_v4(), "plan").unwrap();
        assert_eq!(native.options.effort.as_deref(), expected, "{original}");
        assert_eq!(
            accepted.capture, original,
            "selection cannot rewrite retained bytes"
        );
    }
}
