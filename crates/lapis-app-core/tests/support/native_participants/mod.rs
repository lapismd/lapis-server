#[path = "../native_turn/mod.rs"]
mod native;
use lapis_app_core::services::{ParticipantMessage, ParticipantMessageInput, Runtime};
use lapis_app_protocol::turns::TurnRecord;
pub use native::Fixture;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;

pub fn input(f: &Fixture, count: usize) -> ParticipantMessageInput {
    let fixture: Value =
        serde_json::from_str(include_str!("../../fixtures/participant-capture-v1.json")).unwrap();
    let mut dispatch = fixture["cases"]
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
    ParticipantMessageInput {
        acceptance_id: Uuid::new_v4(),
        profiles: None,
        user: json!({"schemaVersion":3,"id":dispatch["userEntryId"],"type":"message","role":"user","text":"Authored café 📝","createdAt":dispatch["startedAt"],"recipients":recipients}),
        dispatch,
        turns: (0..count)
            .map(|_| {
                let mut t = f.input();
                t.context = None;
                t
            })
            .collect(),
    }
}
pub fn peer(f: &Fixture) {
    fs::write(&f.policy.script, include_str!("peer.py")).unwrap();
}
pub fn release(f: &Fixture, variant: Option<&str>) {
    fs::write(
        format!(
            "{}.release{}",
            f.policy.log.display(),
            variant.map_or_else(String::new, |v| format!(".{v}"))
        ),
        b"ready",
    )
    .unwrap();
}
pub async fn until(ready: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !ready() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
pub async fn prompted(f: &Fixture, count: usize) {
    f.prompted().await;
    until(|| {
        f.commands()
            .iter()
            .filter(|v| v["type"] == "prompt")
            .count()
            == count
    })
    .await;
}
pub fn stored(f: &Fixture, id: Uuid) -> ParticipantMessage {
    serde_json::from_slice(
        &fs::read(
            f.profile
                .join("participants")
                .join(f.vault.to_string())
                .join(format!("{id}.json")),
        )
        .unwrap(),
    )
    .unwrap()
}
pub async fn settled(f: &Fixture, id: Uuid) -> ParticipantMessage {
    until(|| {
        stored(f, id).deliveries.iter().all(|d| {
            !matches!(
                d.state,
                lapis_app_core::services::ParticipantState::Pending
                    | lapis_app_core::services::ParticipantState::Started
            )
        })
    })
    .await;
    stored(f, id)
}
pub fn rows(f: &Fixture, message: &ParticipantMessage, name: &str) -> Vec<Value> {
    fs::read_to_string(
        f.root
            .join(&message.input.turns[0].scope_dir)
            .join(".lapis/agents/sessions")
            .join(message.conversation_id.to_string())
            .join(name),
    )
    .unwrap()
    .lines()
    .map(|s| serde_json::from_str(s).unwrap())
    .collect()
}
pub async fn first(f: &Fixture, runtime: &Runtime, message: &ParticipantMessage) -> TurnRecord {
    let record = f.finished(runtime, message.deliveries[0].request_id).await;
    assert!(f.conversation(&record, "transcript.jsonl").is_file());
    record
}
pub async fn shutdown(runtime: Arc<Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
pub fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(snapshot(&path));
        } else {
            files.insert(path.clone(), fs::read(path).unwrap());
        }
    }
    files
}
pub fn restore(files: BTreeMap<PathBuf, Vec<u8>>) {
    for (path, bytes) in files {
        fs::write(path, bytes).unwrap();
    }
}
