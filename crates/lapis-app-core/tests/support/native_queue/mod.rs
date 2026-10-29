use lapis_ai_service::{
    Launch,
    runtime::{Environment, PreparedLaunch},
};
use lapis_app_core::{
    services::{AgentPolicy, QueueInput, Runtime, TurnStorage},
    vault::VaultRoot,
};
use lapis_app_protocol::turns::{TurnInput, TurnPhase, TurnRecord, TurnTarget};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

pub struct Fixture {
    pub _temp: tempfile::TempDir,
    pub root: PathBuf,
    pub profile: PathBuf,
    pub vault: Uuid,
    pub policy: Arc<Policy>,
}
pub struct Policy {
    script: PathBuf,
    log: PathBuf,
    pub missing: AtomicBool,
    pub changed: AtomicBool,
}
impl AgentPolicy for Policy {
    fn targets(&self, _: Uuid, _: &VaultRoot) -> lapis_ai_service::Result<Vec<TurnTarget>> {
        Ok(if self.missing.load(Ordering::SeqCst) {
            Vec::new()
        } else {
            vec![TurnTarget {
                queue_selection: None,
                id: "fixture".into(),
                label: "Operator fixture".into(),
                default_mode: if self.changed.load(Ordering::SeqCst) {
                    "changed"
                } else {
                    "ask"
                }
                .into(),
            }]
        })
    }
    fn resolve(
        &self,
        _vault: Uuid,
        scope: &VaultRoot,
        target: &str,
    ) -> lapis_ai_service::Result<Launch> {
        if target != "fixture" {
            return Err(lapis_ai_service::Error("target_unavailable"));
        }
        let mut environment = Environment::default();
        environment
            .0
            .insert("LOG".into(), self.log.to_string_lossy().into());
        environment.0.insert(
            "RELEASE".into(),
            self.log.with_file_name("release").to_string_lossy().into(),
        );
        Ok(Launch {
            prepared: PreparedLaunch {
                executable: self.script.clone(),
                arguments: vec![],
                directory: scope.path().into(),
                environment,
            },
            directory: scope.path().into(),
            resume: None,
        })
    }
}

impl Fixture {
    pub fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let root = base.join("vault");
        let profile = base.join("turns");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&profile).unwrap();
        let script = base.join("queue-peer.py");
        fs::write(&script, include_str!("peer.py")).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        let policy = Arc::new(Policy {
            script,
            log: base.join("commands.jsonl"),
            missing: AtomicBool::new(false),
            changed: AtomicBool::new(false),
        });
        Self {
            _temp: temp,
            root,
            profile,
            vault: Uuid::new_v4(),
            policy,
        }
    }
    pub fn runtime(&self) -> Arc<Runtime> {
        let storage = TurnStorage::new(
            &self.profile,
            self.policy.clone(),
            tokio::runtime::Handle::current(),
        )
        .unwrap();
        let runtime = Arc::new(Runtime::default().with_turns(storage));
        runtime.admit(self.vault, &self.root).unwrap();
        runtime
    }
    pub fn release(&self) {
        fs::write(self.policy.log.with_file_name("release"), b"ready").unwrap();
    }
    pub fn input(&self) -> TurnInput {
        TurnInput {
            request_id: Uuid::new_v4(),
            continuation: None,
            target_id: "fixture".into(),
            scope_dir: "".into(),
            prompt: "Authored café 📝".into(),
            context: Some("Separate context".into()),
            options: Default::default(),
        }
    }
    pub async fn finished(&self, runtime: &Runtime, id: Uuid) -> TurnRecord {
        let client = runtime.get(self.vault).unwrap().attach().unwrap();
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                let record = client.turn(id).unwrap().unwrap();
                if !matches!(record.phase, TurnPhase::Accepted | TurnPhase::Running) {
                    return record;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
    pub async fn prompted(&self) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if self
                    .commands()
                    .iter()
                    .any(|value| value["type"] == "prompt")
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
    pub fn commands(&self) -> Vec<serde_json::Value> {
        fs::read_to_string(&self.policy.log)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }
    pub fn conversation(&self, record: &TurnRecord, name: &str) -> PathBuf {
        self.root
            .join(&record.input.scope_dir)
            .join(".lapis/agents/sessions")
            .join(record.conversation_id.to_string())
            .join(name)
    }
    pub fn queued(&self, head: &TurnRecord, name: &str) -> QueueInput {
        QueueInput {
            acceptance_id: Uuid::new_v4(),
            conversation_id: head.conversation_id,
            after_request_id: head.input.request_id,
            scope_dir: head.input.scope_dir.clone(),
            target_id: "fixture".into(),
            capture: json!({"version":1,"id":format!("capture-{name}"),"state":"queued",
                "user":{"schemaVersion":3,"id":format!("queued-user-{name}"),"type":"message","role":"user",
                    "createdAt":"2026-10-27T00:00:00.000Z","text":format!("Authored {name} café 📝"),
                    "recipients":[{"id":"writer","label":"Writer","token":"jj-store-agent:777269746572;"}],
                    "provenance":{"originClass":"owner","sourceKind":"user-message"}},
                "selection":{"runtime":"acp","thinking":"high","targets":[{"id":"writer","agent":"local","thinking":"default"}],"attachments":[]}}),
        }
    }
    pub async fn queue_terminal(
        &self,
        runtime: &Runtime,
        conversation: Uuid,
        count: usize,
    ) -> lapis_app_core::services::TurnQueue {
        let client = runtime.get(self.vault).unwrap().attach().unwrap();
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                let queue = client.turn_queue(conversation).unwrap().unwrap();
                if queue.entries.iter().filter(|e| e.receipt.is_some()).count() == count {
                    return queue;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
    pub fn transcript(&self, head: &TurnRecord) -> Vec<Value> {
        fs::read_to_string(self.conversation(head, "transcript.jsonl"))
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect()
    }
    pub fn prompt_count(&self) -> usize {
        self.commands()
            .iter()
            .filter(|v| v["type"] == "prompt")
            .count()
    }
}
pub async fn shutdown(runtime: Arc<Runtime>) {
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
