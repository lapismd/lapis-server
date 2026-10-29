mod credentials;
#[path = "support/native_participants/mod.rs"]
#[allow(dead_code)]
mod support;
use lapis_app_protocol::{participants::ParticipantCommand, turns::TurnCommand};
use lapis_server::{
    config::{Config, VaultConfig},
    host::{Host, Principal, turns::AgentConfig},
};
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt};
use uuid::Uuid;

fn selection(id: &str, thinking: &str) -> Value {
    let token = id
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    json!({"recipient":{"id":id,"label":id,"token":format!("jj-store-agent:{token};")},"target":{"id":id,"agent":"codex","thinking":thinking,"model":{"provider":"codex","model":format!("model-{id}")}},"thinking":"default"})
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn operator_catalog_accepts_sixteen_profiles_and_refuses_duplicates_overflow_or_forged_tokens()
 {
    for variant in 0..4 {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let profile = root.join("profile");
        let vault = root.join("vault");
        fs::create_dir(&profile).unwrap();
        fs::create_dir(&vault).unwrap();
        let executable = root.join("peer.py");
        fs::write(&executable, b"#!/usr/bin/python3\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let mut profiles: Vec<_> = (0..16)
            .map(|i| selection(&format!("role-{i}"), "default"))
            .collect();
        match variant {
            1 => profiles.push(selection("overflow", "default")),
            2 => profiles[1] = profiles[0].clone(),
            3 => profiles[0]["recipient"]["token"] = "jj-store-agent:00;".into(),
            _ => {}
        }
        let agent: AgentConfig = serde_json::from_value(json!({"id":"opaque-runtime","label":"Runtime","default_mode":"plan","executable":executable,"participant_selections":profiles})).unwrap();
        let host = Host::with_profile_and_agents(
            Config {
                vaults: vec![VaultConfig {
                    id: Uuid::new_v4(),
                    name: "Notes".into(),
                    root: vault,
                }],
            },
            &profile,
            vec![agent],
        );
        assert_eq!(host.is_ok(), variant == 0, "{variant}");
        if let Ok(host) = host {
            tokio::task::spawn_blocking(move || host.shutdown())
                .await
                .unwrap()
                .unwrap();
        }
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn declared_participant_profiles_preserve_two_logical_roles_on_one_runtime() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let profile = root.join("profile");
    let vault = root.join("vault");
    fs::create_dir(&profile).unwrap();
    fs::create_dir(&vault).unwrap();
    let executable = root.join("peer.py");
    fs::write(&executable, b"#!/usr/bin/python3\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let metadata = json!({"id":"opaque-runtime","label":"Runtime label","default_mode":"plan","executable":executable,"environment":{"PRIVATE_TOKEN":"never-in-profile-records"},"participant_selections":[selection("writer","default"),selection("reviewer","high")]});
    let agent:AgentConfig=serde_json::from_value(metadata).expect("Trusted participant metadata must be admitted without deriving logical IDs from runtime labels");
    let id = Uuid::new_v4();
    let host = Host::with_profile_and_agents(
        Config {
            vaults: vec![VaultConfig {
                id,
                name: "Notes".into(),
                root: vault,
            }],
        },
        &profile,
        vec![agent],
    )
    .unwrap();
    let client = host.attach(id, Principal::Token).unwrap();
    let value = host
        .participant(
            client.client_id,
            Principal::Token,
            ParticipantCommand::Profiles {},
        )
        .unwrap();
    let value = serde_json::to_value(value).unwrap();
    assert_eq!(value["kind"], "profiles");
    let rows = value["profiles"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r["target_id"] == "opaque-runtime"));
    assert_eq!(rows[0]["selection"]["recipient"]["id"], "writer");
    assert_eq!(rows[1]["selection"]["recipient"]["id"], "reviewer");
    assert_eq!(rows[0]["selection"]["target"]["thinking"], "default");
    assert_eq!(rows[0]["options"]["effort"], Value::Null);
    assert_eq!(rows[1]["options"]["effort"], "high");
    assert_eq!(rows[0]["options"]["mode"], "plan");
    assert_eq!(rows[0]["options"]["model"], "model-writer");
    assert!(!value.to_string().contains("never-in-profile-records"));
    assert!(!value.to_string().contains("executable"));
    let targets = host
        .turn(client.client_id, Principal::Token, TurnCommand::Targets {})
        .unwrap();
    assert_eq!(
        serde_json::to_value(targets).unwrap()["targets"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    tokio::task::spawn_blocking(move || host.shutdown())
        .await
        .unwrap()
        .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exact_native_request_discovers_only_its_admitted_participant_receipt() {
    let f = support::Fixture::start(false, true).await;
    support::peer(&f);
    let client = f.attach(0).await;
    let input = support::input(2);
    let accepted =
        support::call(&f, client.client_id, json!({"kind":"accept","input":input})).await;
    f.until(|| f.prompts() == 2).await;
    let response = support::command(
        &f,
        client.client_id,
        json!({"kind":"for_turn","request_id":input["turns"][1]["request_id"]}),
    )
    .send()
    .await
    .unwrap();
    if !response.status().is_success() {
        let status = response.status();
        support::release(&f);
        f.close().await;
        panic!("Exact admitted native request receipt discovery must succeed; got {status}");
    }
    let found: Value = response.json().await.unwrap();
    assert_eq!(
        found["message"]["acceptance_id"],
        accepted["message"]["acceptance_id"]
    );
    assert_eq!(found["message"]["input"], input);
    let other = f.attach(1).await;
    assert_eq!(
        support::call(
            &f,
            other.client_id,
            json!({"kind":"for_turn","request_id":input["turns"][1]["request_id"]})
        )
        .await,
        json!({"kind":"message","message":null})
    );
    assert_eq!(
        support::call(
            &f,
            client.client_id,
            json!({"kind":"for_turn","request_id":Uuid::new_v4()})
        )
        .await,
        json!({"kind":"message","message":null})
    );
    support::release(&f);
    f.until(|| support::terminal(&f, &input["acceptance_id"]))
        .await;
    assert_eq!(f.prompts(), 2);
    f.close().await;
}
