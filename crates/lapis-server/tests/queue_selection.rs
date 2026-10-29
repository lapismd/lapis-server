use lapis_server::{
    config::Config,
    host::{Host, turns::AgentConfig},
};
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt};

fn selection() -> Value {
    json!({
        "recipient":{"id":"writer","label":"Writer","token":"jj-store-agent:777269746572;"},
        "target":{"id":"writer","agent":"codex","thinking":"default"}
    })
}
#[tokio::test]
async fn operator_queue_capture_metadata_is_validated_before_native_host_admission() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let profile = base.join("profile");
    fs::create_dir(&profile).unwrap();
    let helper = base.join("helper");
    fs::write(&helper, "fixture").unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
    let config = json!({"id":"opaque","label":"Display name","default_mode":"read-only","executable":helper,"queue_selection":selection()});
    let agent: AgentConfig = serde_json::from_value(config.clone()).unwrap();
    assert!(
        Host::with_profile_and_agents(Config { vaults: vec![] }, &profile, vec![agent]).is_ok()
    );
    for (path, invalid) in [
        (
            vec!["recipient", "token"],
            json!("jj-store-agent:6f74686572;"),
        ),
        (vec!["recipient", "label"], json!("\n")),
        (vec!["target", "id"], json!("different")),
        (vec!["target", "thinking"], json!("not-a-level")),
        (vec!["target", "agent"], json!("not-an-agent")),
    ] {
        let mut invalid_config = config.clone();
        invalid_config["queue_selection"][path[0]][path[1]] = invalid;
        let result = serde_json::from_value::<AgentConfig>(invalid_config).map(|agent| {
            Host::with_profile_and_agents(Config { vaults: vec![] }, &profile, vec![agent]).is_err()
        });
        assert!(result.is_err() || result.unwrap());
    }
}
