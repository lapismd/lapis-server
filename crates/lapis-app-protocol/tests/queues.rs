use lapis_app_protocol::queues::*;
use serde_json::json;
use ts_rs::{Config, TS};
#[test]
fn queue_dtos_round_trip_complete_data_and_refuse_launch_or_scope_authority() {
    let id = "8812d6cc-5e77-4239-9010-4385ccbb4252";
    let input = json!({"acceptance_id":id,"conversation_id":id,"after_request_id":id,"scope_dir":"Notes","target_id":"opaque-native-target","capture":{"version":1,"user":{"id":"queued-user-authored","text":"café 📝"},"selection":{"thinking":"default"}}});
    let value = json!({"kind":"accept","input":input});
    let command: QueueCommand = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(command).unwrap(), value);
    for kind in ["read", "stop"] {
        let v = json!({"kind":kind,"conversation_id":id});
        let c: QueueCommand = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(c).unwrap(), v);
    }
    for v in [
        json!({"kind":"cancel","conversation_id":id,"acceptance_id":id}),
        json!({"kind":"resume","conversation_id":id,"after_request_id":id}),
    ] {
        let c: QueueCommand = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(c).unwrap(), v);
    }
    for key in [
        "vault_id",
        "root",
        "executable",
        "environment",
        "resume_session_id",
    ] {
        let mut v = value.clone();
        v[key] = json!("forged");
        assert!(serde_json::from_value::<QueueCommand>(v).is_err());
        let mut v = value.clone();
        v["input"][key] = json!("forged");
        assert!(serde_json::from_value::<QueueCommand>(v).is_err());
    }
    for v in [
        json!({"kind":"resume","conversation_id":id}),
        json!({"kind":"cancel","conversation_id":id}),
        json!({"kind":"accept","input":{"capture":{}}}),
        json!({"kind":"read","conversation_id":"not-a-uuid"}),
        json!({"kind":"execute"}),
    ] {
        assert!(serde_json::from_value::<QueueCommand>(v).is_err());
    }
    let cfg = Config::default();
    assert!(QueueCommand::decl(&cfg).contains("\"resume\""));
    assert!(QueueAccept::decl(&cfg).contains("capture: unknown"));
    assert!(!QueueEntry::decl(&cfg).contains("default_mode"));
    let schema = serde_json::to_value(schemars::schema_for!(QueueCommand)).unwrap();
    assert!(schema.to_string().contains("additionalProperties"));
}
