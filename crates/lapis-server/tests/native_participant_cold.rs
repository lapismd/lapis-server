mod credentials;
#[path = "support/native_participants/mod.rs"]
#[allow(dead_code)]
mod support;
use serde_json::json;
use std::fs;
use support::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn participant_cold_read_retains_confirmed_and_unknown_without_replay_or_portable_writes() {
    let mut f = Fixture::start(false, true).await;
    peer(&f);
    let client = f.attach(0).await;
    let mut input = input(2);
    input["turns"][0]["options"]["model"] = json!("ordinary");
    let accepted = call(&f, client.client_id, json!({"kind":"accept","input":input})).await;
    f.until(|| {
        stored(&f, &input["acceptance_id"]).is_some_and(|m| {
            m["deliveries"][0]["state"] == "completed" && m["deliveries"][1]["state"] == "started"
        })
    })
    .await;
    let checkpoint = f.temp.path().join("cold-checkpoint");
    copy_tree(&f.profile.join("turns"), &checkpoint.join("turns"));
    copy_tree(&f.temp.path().join("vault-0"), &checkpoint.join("vault"));
    let files = ["transcript.jsonl", "metadata.yaml", "agents.jsonl"];
    let directory = checkpoint
        .join("vault/.lapis/agents/sessions")
        .join(accepted["message"]["conversation_id"].as_str().unwrap());
    let before: Vec<_> = files
        .iter()
        .map(|name| fs::read(directory.join(name)).unwrap())
        .collect();
    f.close().await;
    for (source, destination) in [
        (checkpoint.join("turns"), f.profile.join("turns")),
        (checkpoint.join("vault"), f.temp.path().join("vault-0")),
    ] {
        fs::remove_dir_all(&destination).unwrap();
        copy_tree(&source, &destination);
    }
    f.restart().await;
    let client = f.attach(0).await;
    let read = call(
        &f,
        client.client_id,
        json!({"kind":"read","acceptance_id":input["acceptance_id"]}),
    )
    .await;
    assert_eq!(read["message"]["recovered"], true);
    assert_eq!(read["message"]["input"], input);
    assert_eq!(read["message"]["deliveries"][0]["state"], "completed");
    assert_eq!(read["message"]["deliveries"][1]["state"], "unknown");
    for (i, d) in accepted["message"]["deliveries"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        for key in [
            "delivery_id",
            "request_id",
            "agent_binding_id",
            "session_id",
        ] {
            assert_eq!(read["message"]["deliveries"][i][key], d[key]);
        }
    }
    assert_eq!(
        call(&f, client.client_id, json!({"kind":"accept","input":input})).await,
        read
    );
    assert_eq!(
        call(
            &f,
            client.client_id,
            json!({"kind":"stop","acceptance_id":input["acceptance_id"]})
        )
        .await,
        json!({"kind":"stop_requested","accepted":false})
    );
    let directory = f
        .temp
        .path()
        .join("vault-0/.lapis/agents/sessions")
        .join(accepted["message"]["conversation_id"].as_str().unwrap());
    for (i, name) in files.iter().enumerate() {
        assert_eq!(fs::read(directory.join(name)).unwrap(), before[i]);
    }
    assert_eq!(f.prompts(), 2);
    f.close().await;
}
