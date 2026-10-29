use lapis_app_protocol::turns::*;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

fn input() -> Value {
    json!({"request_id":"8812d6cc-5e77-4239-9010-4385ccbb4252", "target_id":"codex",
        "scope_dir":"Notes", "prompt":"Authored café 📝", "context":"Separate context"})
}

#[test]
fn turn_commands_round_trip_with_portable_options_and_exact_discriminants() {
    let command: TurnCommand =
        serde_json::from_value(json!({"kind":"submit", "input":input()})).unwrap();
    let TurnCommand::Submit { input } = &command else {
        panic!()
    };
    assert_eq!(input.options.mode, "ask");
    assert_eq!(input.options.max_duration_ms, 60_000);
    let encoded = serde_json::to_value(&command).unwrap();
    assert_eq!(encoded["input"]["prompt"], "Authored café 📝");
    assert_eq!(encoded["input"]["context"], "Separate context");
    let cfg = Config::default();
    assert!(TurnCommand::decl(&cfg).contains("\"submit\""));
    assert!(TurnOptions::decl(&cfg).contains("max_duration_ms: number"));
    assert!(TurnRecord::decl(&cfg).contains("accepted_at_ms: number"));
    assert!(!TurnSummary::decl(&cfg).contains("prompt"));
    assert!(!TurnSummary::decl(&cfg).contains("output"));
    for kind in ["read", "cancel"] {
        let value = json!({"kind":kind,"request_id":input.request_id});
        let parsed: TurnCommand = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), value);
    }
}

#[test]
fn feature_activation_is_strict_boolean_without_native_launch_authority() {
    for enabled in [false, true] {
        let value = json!({"kind":"set_enabled", "enabled":enabled});
        let command: TurnCommand = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(command).unwrap(), value);
        let value = json!({"kind":"availability", "enabled":enabled});
        let response: TurnValue = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(response).unwrap(), value);
    }
    for value in [
        json!({"kind":"set_enabled"}),
        json!({"kind":"set_enabled", "enabled":"true"}),
        json!({"kind":"set_enabled", "enabled":true, "root":"/tmp"}),
    ] {
        assert!(serde_json::from_value::<TurnCommand>(value).is_err());
    }
}

#[test]
fn continuation_cursor_round_trips_without_admitting_native_resume_authority() {
    let original: TurnInput = serde_json::from_value(input()).unwrap();
    assert!(original.continuation.is_none());
    assert!(
        serde_json::to_value(original)
            .unwrap()
            .get("continuation")
            .is_none()
    );
    let mut value = input();
    value["continuation"] = json!({
        "conversation_id":"24706af7-64b4-47c0-8cf4-1157161d7b33",
        "after_request_id":"af2cad91-6e47-428a-8be7-7a19a6fb812c"
    });
    let continued: TurnInput = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(continued).unwrap()["continuation"],
        value["continuation"]
    );
    value["continuation"]["resume"] = json!("untrusted-native-session");
    assert!(serde_json::from_value::<TurnInput>(value).is_err());
}

#[test]
fn turn_wire_never_admits_native_launch_fields_or_unknown_options() {
    for field in ["root", "executable", "environment", "arguments", "resume"] {
        let mut value = input();
        value[field] = json!("untrusted");
        assert!(
            serde_json::from_value::<TurnInput>(value).is_err(),
            "{field}"
        );
        let value = json!({"kind":"list",field:"untrusted"});
        assert!(
            serde_json::from_value::<TurnCommand>(value).is_err(),
            "{field}"
        );
    }
    let mut value = input();
    value["options"] = json!({"model":null,"effort":null,"mode":"ask",
        "max_duration_ms":60_000,"approvalPolicy":"allow-all"});
    assert!(serde_json::from_value::<TurnInput>(value).is_err());
    assert!(serde_json::from_value::<TurnCommand>(json!({"kind":"unknown"})).is_err());
    assert!(
        serde_json::from_value::<TurnValue>(json!({"kind":"records","records":[],"root":"/tmp"}))
            .is_err()
    );
    let schema = serde_json::to_value(schemars::schema_for!(TurnCommand)).unwrap();
    for variant in schema["oneOf"].as_array().unwrap() {
        assert_eq!(variant["additionalProperties"], false);
    }
}
