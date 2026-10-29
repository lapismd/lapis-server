use lapis_app_protocol::participants::*;
use serde_json::{Value, json};
use ts_rs::{Config, TS};
use uuid::Uuid;

fn input() -> Value {
    json!({"acceptance_id":Uuid::new_v4(),"user":{"schemaVersion":3,"id":"authored:parent","text":"Café 📝","recipients":[{"id":"reviewer","label":"Equal label","token":"jj-store-agent:7265766965776572;"}]},"dispatch":{"id":"captured:dispatch","deliveries":[{"id":"captured:delivery","recipient":{"id":"reviewer","label":"Equal label","token":"jj-store-agent:7265766965776572;"},"state":"pending"}]},"turns":[{"request_id":Uuid::new_v4(),"target_id":"installed-runtime","scope_dir":"Notes","prompt":"Café 📝","context":null}]})
}
#[test]
fn participant_commands_and_all_outcomes_round_trip_complete_capture_and_native_ids() {
    let original = input();
    let command: ParticipantCommand =
        serde_json::from_value(json!({"kind":"accept","input":original})).unwrap();
    let encoded = serde_json::to_value(command).unwrap();
    assert_eq!(encoded["input"]["user"], original["user"]);
    assert_eq!(encoded["input"]["dispatch"], original["dispatch"]);
    assert_eq!(encoded["input"]["turns"][0]["options"]["mode"], "ask");
    let acceptance = original["acceptance_id"].clone();
    for kind in ["read", "stop"] {
        let value = json!({"kind":kind,"acceptance_id":acceptance});
        let parsed: ParticipantCommand = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), value);
    }
    let value = json!({"kind":"cancel","acceptance_id":acceptance,"request_id":original["turns"][0]["request_id"]});
    let parsed: ParticipantCommand = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);
    for state in [
        "pending",
        "started",
        "completed",
        "rejected",
        "unknown",
        "not_sent",
        "cancelled",
    ] {
        let value = json!({"kind":"message","message":{"schema_version":1,"vault_id":Uuid::new_v4(),"acceptance_id":acceptance,"conversation_id":Uuid::new_v4(),"input":encoded["input"],"deliveries":[{"delivery_id":"captured:delivery","request_id":original["turns"][0]["request_id"],"agent_binding_id":Uuid::new_v4(),"session_id":Uuid::new_v4(),"state":state,"outcome_published":state=="completed"}],"recovered":true}});
        let parsed: ParticipantValue = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), value);
    }
    for value in [
        json!({"kind":"profiles","profiles":[]}),
        json!({"kind":"message","message":null}),
        json!({"kind":"stop_requested","accepted":false}),
        json!({"kind":"cancel_requested","accepted":true}),
    ] {
        let parsed: ParticipantValue = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), value);
    }
}
#[test]
fn participant_frames_reject_native_authority_and_declare_unknown_captures_without_any() {
    let cfg = Config::default();
    let declaration = ParticipantMessageInput::decl(&cfg);
    assert!(declaration.contains("user: unknown"));
    assert!(declaration.contains("dispatch: unknown"));
    assert!(!declaration.contains("any"));
    assert!(ParticipantCommand::decl(&cfg).contains("request_id: string"));
    assert_eq!(PARTICIPANT_REQUEST_LIMIT, 2 * 1024 * 1024);
    assert_eq!(PARTICIPANT_RECORD_LIMIT, 8 * 1024 * 1024);
    assert_eq!(PARTICIPANT_PROFILE_LIMIT, 16);
    for key in ["executable", "environment", "root", "resume", "arguments"] {
        let mut value = input();
        value[key] = json!("foreign");
        assert!(
            serde_json::from_value::<ParticipantCommand>(json!({"kind":"accept","input":value}))
                .is_err(),
            "{key}"
        );
    }
    for value in [
        json!({"kind":"resume","acceptance_id":Uuid::new_v4()}),
        json!({"kind":"cancel","acceptance_id":Uuid::new_v4()}),
        json!({"kind":"read","acceptance_id":"authored:parent"}),
        json!({"kind":"stop","acceptance_id":Uuid::new_v4(),"target_id":"foreign"}),
        json!({"kind":"for_turn","acceptance_id":Uuid::new_v4()}),
        json!({"kind":"for_turn","request_id":"authored-parent"}),
    ] {
        assert!(serde_json::from_value::<ParticipantCommand>(value).is_err());
    }
}

#[test]
fn complete_selected_profile_data_and_exact_lookup_round_trip_without_native_authority() {
    let selection = json!({"recipient":{"id":"reviewer","label":"Equal label","token":"jj-store-agent:7265766965776572;"},"target":{"id":"reviewer","agent":"codex","model":{"provider":"codex","model":"model-reviewer"},"thinking":"default"},"thinking":"high"});
    let profile = json!({"target_id":"installed-runtime","selection":selection,"options":{"mode":"plan","model":"model-reviewer","effort":null,"max_duration_ms":60000}});
    let parsed: ParticipantProfile = serde_json::from_value(profile.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), profile);
    let mut capture = input();
    capture["profiles"] = json!([profile]);
    let parsed: ParticipantCommand =
        serde_json::from_value(json!({"kind":"accept","input":capture})).unwrap();
    assert_eq!(
        serde_json::to_value(parsed).unwrap()["input"]["profiles"],
        capture["profiles"]
    );
    for value in [
        json!({"kind":"profiles"}),
        json!({"kind":"for_turn","request_id":Uuid::new_v4()}),
    ] {
        let parsed: ParticipantCommand = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), value);
    }
    for key in ["executable", "environment", "provider_session"] {
        let mut forged = profile.clone();
        forged[key] = json!("forged");
        assert!(serde_json::from_value::<ParticipantProfile>(forged).is_err());
    }
}
