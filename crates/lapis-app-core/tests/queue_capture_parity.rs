use lapis_app_core::turns::queue_capture::{
    configuration_frame, create_receipt, validate_message, validate_queue, validate_receipt,
};
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/queue-capture-v1.json")).unwrap()
}
#[test]
fn retained_queue_capture_complete_source_results() {
    let fixture = fixture();
    for case in fixture["cases"].as_array().unwrap() {
        let input = case.get("input").cloned();
        let result = match case["action"].as_str().unwrap() {
            "message" => validate_message(input.unwrap_or(Value::Null)).map(|v| v.into_value()),
            "queue" => validate_queue(input).map(|v| serde_json::to_value(v).unwrap()),
            "receipt" => validate_receipt(input.unwrap_or(Value::Null)).map(|v| v.into_value()),
            _ => panic!("unknown source action"),
        };
        let actual = match result {
            Ok(value) => json!({"ok":true,"value":value}),
            Err(error) => json!({"ok":false,"error":error}),
        };
        assert_eq!(actual, case["expected"], "{}", case["name"]);
        if case["action"] == "message" && case["expected"]["ok"] == true {
            let capture = validate_message(case["input"].clone()).unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&capture.encode()).unwrap(),
                case["expected"]["value"]
            );
        }
    }
}
#[test]
fn retained_runtime_configuration_uses_actual_source_frames() {
    for case in fixture()["configurationFrames"].as_array().unwrap() {
        assert_eq!(
            configuration_frame(&case["input"], "codex").unwrap(),
            case["frame"]
        );
    }
}
#[test]
fn constructed_receipts_keep_capture_and_authored_parent_identities() {
    for case in fixture()["cases"].as_array().unwrap().iter().filter(|c| {
        c["name"].as_str().unwrap().starts_with("receipt-")
            && c["name"].as_str().unwrap().split('-').count() == 2
    }) {
        let expected = &case["expected"]["value"];
        let capture = validate_message(expected["queued"].clone()).unwrap();
        let receipt = create_receipt(
            &capture,
            expected["outcome"].as_str().unwrap(),
            expected["createdAt"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(receipt.as_value(), expected);
    }
}
