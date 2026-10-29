mod commands;
mod lifecycle;
use super::*;
use crate::{
    embedding::host::NativeEmbeddingHost,
    services::{DatabaseStorage, Runtime},
};
use lapis_app_protocol::TextSnapshot;
use serde_json::{Value, json};
use std::fs;
use uuid::Uuid;

struct Fixture {
    _temp: tempfile::TempDir,
    root: std::path::PathBuf,
    profile: std::path::PathBuf,
    id: Uuid,
    case: Value,
}
impl Fixture {
    fn new(name: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let root = base.join("vault");
        let profile = base.join("profile");
        fs::create_dir_all(root.join("tasks")).unwrap();
        fs::create_dir_all(profile.join("tasks")).unwrap();
        let corpus: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/task-lifecycle.json"
        )))
        .unwrap();
        let case = corpus["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["name"] == name)
            .unwrap()
            .clone();
        fs::write(root.join("tasks/Read.md"), case["raw"].as_str().unwrap()).unwrap();
        for (path, text) in case["daily"].as_object().unwrap() {
            fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            fs::write(root.join(path), text.as_str().unwrap()).unwrap();
        }
        Self {
            _temp: temp,
            root,
            profile,
            id: Uuid::new_v4(),
            case,
        }
    }
    fn runtime(&self) -> Runtime {
        Runtime::with_database(
            DatabaseStorage::new(&self.profile, NativeEmbeddingHost::default()).unwrap(),
        )
        .with_tasks(TaskStorage::new(&self.profile.join("tasks")).unwrap())
    }
    fn configuration(&self) -> NativeTaskConfiguration {
        serde_json::from_value(json!({"day_parts":self.case["settings"],"daily":{"folder":"daily","date_format":"yyyy-LL-dd"},"format":{"locale":"en-US","firstDay":7,"minimalDays":4},"provider":{"kind":"default"}})).unwrap()
    }
    fn request(&self, vault: &Arc<VaultService>) -> NativeTaskLifecycleRequest {
        NativeTaskLifecycleRequest {
            request_id: Uuid::new_v4(),
            path: "tasks/Read.md".into(),
            expected_version: snapshot(vault).version,
            action: serde_json::from_value(self.case["actions"][0].clone()).unwrap(),
        }
    }
}
fn snapshot(vault: &Arc<VaultService>) -> TextSnapshot {
    vault
        .begin_work()
        .unwrap()
        .read_text("tasks/Read.md")
        .unwrap()
}
async fn close(runtime: Runtime) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
async fn configured(f: &Fixture, runtime: &Runtime) -> (Arc<VaultService>, Client) {
    let vault = runtime.admit(f.id, &f.root).unwrap();
    let client = vault.attach().unwrap();
    let NativeTaskValue::Status { status } = client
        .control_tasks(NativeTaskCommand::Status {})
        .await
        .unwrap()
    else {
        panic!("status")
    };
    assert!(!status.enabled);
    let NativeTaskValue::Status { status } = client
        .control_tasks(NativeTaskCommand::Configure {
            configuration: f.configuration(),
        })
        .await
        .unwrap()
    else {
        panic!("configure")
    };
    assert!(status.enabled);
    (vault, client)
}
