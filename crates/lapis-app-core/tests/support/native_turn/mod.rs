use lapis_ai_service::{
    Launch,
    runtime::{Environment, PreparedLaunch},
};
use lapis_app_core::{
    services::{AgentPolicy, Runtime, TurnStorage},
    vault::VaultRoot,
};
use lapis_app_protocol::turns::{TurnInput, TurnPhase, TurnRecord};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

pub struct Policy {
    pub script: PathBuf,
    pub log: PathBuf,
    pub calls: AtomicUsize,
    pub stalled_capture: AtomicBool,
    pub configuration_unavailable: AtomicBool,
}
impl AgentPolicy for Policy {
    fn resolve(
        &self,
        _: Uuid,
        scope: &VaultRoot,
        target: &str,
    ) -> lapis_ai_service::Result<Launch> {
        if target != "fixture" {
            return Err(lapis_ai_service::Error("target_unavailable"));
        }
        if self.configuration_unavailable.load(Ordering::SeqCst) {
            return Err(lapis_ai_service::Error("configuration_unavailable"));
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut environment = Environment::default();
        environment
            .0
            .insert("LOG".into(), self.log.to_string_lossy().into());
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
    fn processor(
        &self,
        _: Uuid,
        _: &lapis_ai_service::SessionBinding,
    ) -> lapis_ai_service::Result<Option<Box<dyn lapis_ai_service::runtime::SessionProcessor>>>
    {
        Ok(self.stalled_capture.load(Ordering::SeqCst).then(|| {
            Box::new(StalledCapture) as Box<dyn lapis_ai_service::runtime::SessionProcessor>
        }))
    }
}
struct StalledCapture;
impl lapis_ai_service::runtime::SessionProcessor for StalledCapture {
    fn process<'a>(
        &'a mut self,
        input: lapis_ai_service::runtime::FrameInput<'a>,
    ) -> lapis_ai_service::runtime::ProcessorFuture<'a> {
        Box::pin(async move {
            if input.frame["type"] == "activity" {
                tokio::time::sleep(Duration::from_millis(180)).await;
            }
            Ok(input.frame.clone())
        })
    }
}
pub struct Fixture {
    pub _temp: tempfile::TempDir,
    pub root: PathBuf,
    pub profile: PathBuf,
    pub vault: Uuid,
    pub policy: Arc<Policy>,
}
impl Fixture {
    pub fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let root = base.join("vault");
        let profile = base.join("turns");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&profile).unwrap();
        let script = base.join("peer.py");
        fs::write(&script, include_str!("peer.py")).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        let policy = Arc::new(Policy {
            script,
            log: base.join("commands.jsonl"),
            calls: AtomicUsize::new(0),
            stalled_capture: AtomicBool::new(false),
            configuration_unavailable: AtomicBool::new(false),
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
}
