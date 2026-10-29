use lapis_app_core::turns::participant_capture::{
    delivery_entry, validate_delivery, validate_dispatch,
};
use serde_json::{Value, json};
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/participant-capture-v1.json")).unwrap()
}
#[test]
fn participant_capture_retains_complete_source_validator_results() {
    for case in fixture()["cases"].as_array().unwrap() {
        let result = match case["action"].as_str().unwrap() {
            "dispatch" => validate_dispatch(case["input"].clone()),
            "delivery" => validate_delivery(case["input"].clone()),
            _ => panic!("unknown source action"),
        };
        let actual = match result {
            Ok(value) => json!({"ok":true,"value":value}),
            Err(error) => json!({"ok":false,"error":error}),
        };
        assert_eq!(actual, case["expected"], "{}", case["name"]);
    }
}
#[test]
fn participant_outcome_entries_retain_exact_source_parent_dispatch_and_binding() {
    for case in fixture()["entries"].as_array().unwrap() {
        assert_eq!(
            delivery_entry(
                &case["dispatch"],
                &case["delivery"],
                case["entry"]["createdAt"].as_str().unwrap()
            )
            .unwrap(),
            case["entry"]
        );
    }
}
