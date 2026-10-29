use lapis_server::{
    config::Config,
    host::{Host, turns::AgentConfig},
};
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt};

#[test]
fn operator_default_mode_is_required_and_typed() {
    let base = json!({"id":"opaque-target","label":"Agent","executable":"/helper"});
    assert!(serde_json::from_value::<AgentConfig>(base.clone()).is_err());
    for mode in [json!(null), json!(7), json!([])] {
        let mut value = base.clone();
        value["default_mode"] = mode;
        assert!(serde_json::from_value::<AgentConfig>(value).is_err());
    }
}

#[tokio::test]
async fn operator_default_mode_rejects_invalid_values_before_admission() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let profile = base.join("profile");
    fs::create_dir(&profile).unwrap();
    let helper = base.join("helper");
    fs::write(&helper, "fixture").unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
    for mode in [
        "".to_owned(),
        "  ".into(),
        "plan\n".into(),
        " plan".into(),
        "\u{85}".into(),
        "x".repeat(257),
        "é".repeat(129),
    ] {
        let agent: AgentConfig = serde_json::from_value(json!({
            "id":"opaque-target", "label":"Agent", "executable":helper,
            "default_mode":mode,
        }))
        .unwrap();
        let error = Host::with_profile_and_agents(Config { vaults: vec![] }, &profile, vec![agent])
            .err()
            .expect("invalid mode must reject operator startup");
        assert_eq!(error.to_string(), "Invalid native agent installation");
    }
    let agent: AgentConfig = serde_json::from_value(json!({
        "id":"opaque-target", "label":"Agent", "executable":helper,
        "default_mode":"read-only",
    }))
    .unwrap();
    assert!(
        Host::with_profile_and_agents(Config { vaults: vec![] }, &profile, vec![agent]).is_ok()
    );
}
