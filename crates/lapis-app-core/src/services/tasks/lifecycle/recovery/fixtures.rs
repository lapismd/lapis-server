use super::*;
use crate::services::Runtime;
use serde_json::Value;
use std::{fs, path::PathBuf};

pub(super) struct Fixture {
    _temp: tempfile::TempDir,
    pub(super) root: PathBuf,
    pub(super) profile: PathBuf,
    pub(super) id: Uuid,
    pub(super) case: Value,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let root = base.join("vault");
        let profile = base.join("tasks");
        fs::create_dir_all(root.join("tasks")).unwrap();
        fs::create_dir(&profile).unwrap();
        let fixtures: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/task-lifecycle.json"
        )))
        .unwrap();
        let case = fixtures["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == "complete / schedule / count")
            .unwrap()
            .clone();
        fs::write(root.join("tasks/Read.md"), case["raw"].as_str().unwrap()).unwrap();
        for (path, text) in case["daily"].as_object().unwrap() {
            let target = root.join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, text.as_str().unwrap()).unwrap();
        }
        Self {
            _temp: temp,
            root,
            profile,
            id: Uuid::new_v4(),
            case,
        }
    }
    pub(super) fn vault(&self) -> Arc<VaultService> {
        Runtime::default().admit(self.id, &self.root).unwrap()
    }
    pub(super) fn policy(&self) -> TaskLifecyclePolicy {
        TaskLifecyclePolicy {
            day_parts: serde_json::from_value(self.case["settings"].clone()).unwrap(),
            daily: DailyDocumentSettings::default(),
            format: DailyFormatContext {
                locale: "en-US".into(),
                first_day: 7,
                minimal_days: 4,
            },
        }
    }
    pub(super) fn request(&self, vault: &Arc<VaultService>) -> TaskLifecycleRequest {
        TaskLifecycleRequest {
            request_id: Uuid::new_v4(),
            path: "tasks/Read.md".into(),
            expected_version: vault
                .begin_work()
                .unwrap()
                .read_text("tasks/Read.md")
                .unwrap()
                .version,
            action: serde_json::from_value(self.case["actions"][0].clone()).unwrap(),
        }
    }
    pub(super) fn daily(&self) -> String {
        fs::read_to_string(self.root.join("daily/2026-08-20.md")).unwrap()
    }
    pub(super) fn task(&self) -> String {
        fs::read_to_string(self.root.join("tasks/Read.md")).unwrap()
    }
}
