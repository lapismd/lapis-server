use lapis_server::{
    config::{Config, VaultConfig},
    host::{Host, turns::AgentConfig},
};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};
use uuid::Uuid;

#[tokio::test]
async fn native_agent_installation_rejects_vault_helpers_links_and_duplicate_ids() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let profile = base.join("profile");
    let root = base.join("vault");
    fs::create_dir(&profile).unwrap();
    fs::create_dir(&root).unwrap();
    let helper = base.join("helper");
    fs::write(&helper, "fixture").unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
    let config = || Config {
        vaults: vec![VaultConfig {
            id: Uuid::new_v4(),
            name: "Notes".into(),
            root: root.clone(),
        }],
    };
    let agent = |path: std::path::PathBuf| AgentConfig {
        participant_selections: Vec::new(),
        queue_selection: None,
        id: "fixture".into(),
        label: "Fixture".into(),
        default_mode: "ask".into(),
        executable: path,
        environment: Default::default(),
    };
    let in_vault = root.join("helper");
    fs::copy(&helper, &in_vault).unwrap();
    assert!(Host::with_profile_and_agents(config(), &profile, vec![agent(in_vault)]).is_err());
    let managed = profile.join("vaults");
    fs::create_dir(&managed).unwrap();
    let in_managed = managed.join("helper");
    fs::copy(&helper, &in_managed).unwrap();
    assert!(Host::with_profile_and_agents(config(), &profile, vec![agent(in_managed)]).is_err());
    let linked = base.join("linked");
    symlink(&helper, &linked).unwrap();
    assert!(Host::with_profile_and_agents(config(), &profile, vec![agent(linked)]).is_err());
    assert!(
        Host::with_profile_and_agents(
            config(),
            &profile,
            vec![agent(helper.clone()), agent(helper.clone())]
        )
        .is_err()
    );
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(Host::with_profile_and_agents(config(), &profile, vec![agent(helper)]).is_err());
    assert!(
        serde_json::from_value::<AgentConfig>(
            serde_json::json!({"id":"fixture","label":"Fixture","default_mode":"ask",
        "executable":"/helper","arguments":["--unsafe"]})
        )
        .is_err()
    );
}
