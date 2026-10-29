#[path = "../native_queue/mod.rs"]
#[allow(dead_code)]
mod queue;
pub use queue::Fixture;
pub const ORIGIN: &str = queue::ORIGIN;
pub const TOKEN: &str = queue::TOKEN;
use serde_json::{Value, json};
use std::fs;
use uuid::Uuid;

pub fn peer(f: &Fixture) {
    fs::write(
        f.temp.path().join("peer.py"),
        include_str!("../../../../lapis-app-core/tests/support/native_participants/peer.py"),
    )
    .unwrap();
}
pub fn input(count: usize) -> Value {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../../lapis-app-core/tests/fixtures/participant-capture-v1.json"
    ))
    .unwrap();
    let mut dispatch = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "dispatch-four")
        .unwrap()["expected"]["value"]
        .clone();
    dispatch["deliveries"]
        .as_array_mut()
        .unwrap()
        .truncate(count);
    let recipients: Vec<_> = dispatch["deliveries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["recipient"].clone())
        .collect();
    let user = json!({"schemaVersion":3,"id":dispatch["userEntryId"],"type":"message","role":"user","text":"Authored café 📝","createdAt":dispatch["startedAt"],"recipients":recipients});
    let turns: Vec<_> = (0..count).map(|i| json!({"request_id":Uuid::new_v4(),"target_id":"fixture","scope_dir":"","prompt":user["text"],"context":null,"options":{"model":format!("held-{i}"),"effort":null,"mode":"plan","max_duration_ms":60000}})).collect();
    json!({"acceptance_id":Uuid::new_v4(),"user":user,"dispatch":dispatch,"turns":turns})
}
pub fn command(f: &Fixture, client: Uuid, value: Value) -> reqwest::RequestBuilder {
    f.auth(f.raw(&format!("/api/v3/clients/{client}/participants")))
        .json(&value)
}
pub async fn call(f: &Fixture, client: Uuid, value: Value) -> Value {
    let response = command(f, client, value).send().await.unwrap();
    let status = response.status();
    let value: Value = response.json().await.unwrap();
    assert!(status.is_success(), "{status}: {value}");
    value
}
pub fn stored(f: &Fixture, id: &Value) -> Option<Value> {
    fs::read(
        f.profile
            .join("turns/participants")
            .join(f.ids[0].to_string())
            .join(format!("{}.json", id.as_str().unwrap())),
    )
    .ok()
    .and_then(|s| serde_json::from_slice(&s).ok())
}
pub fn terminal(f: &Fixture, id: &Value) -> bool {
    stored(f, id).is_some_and(|m| {
        m["deliveries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| !matches!(d["state"].as_str(), Some("pending" | "started")))
    })
}
pub fn release(f: &Fixture) {
    fs::write(f.temp.path().join("commands.jsonl.release"), b"ready").unwrap();
}
pub fn rows(f: &Fixture, message: &Value, name: &str) -> Vec<Value> {
    fs::read_to_string(
        f.temp
            .path()
            .join("vault-0/.lapis/agents/sessions")
            .join(message["conversation_id"].as_str().unwrap())
            .join(name),
    )
    .unwrap()
    .lines()
    .map(|s| serde_json::from_str(s).unwrap())
    .collect()
}

pub fn copy_tree(source: &std::path::Path, destination: &std::path::Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        assert!(!entry.file_type().unwrap().is_symlink());
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
