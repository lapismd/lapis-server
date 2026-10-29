mod queue_selection;
use lapis_ai_service::{
    Launch,
    runtime::{Environment, PreparedLaunch},
};
use lapis_app_core::{services::AgentPolicy, vault::VaultRoot};
use lapis_app_protocol::turns::TurnTarget;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashSet},
    io,
    path::PathBuf,
};
use uuid::Uuid;

/// Operator configuration only. Never deserialized by a browser route.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentConfig {
    #[serde(default)]
    pub queue_selection: Option<lapis_app_protocol::queues::QueueSelection>,
    #[serde(default)]
    pub participant_selections: Vec<lapis_app_protocol::queues::QueueSelection>,
    pub id: String,
    pub label: String,
    pub default_mode: String,
    pub executable: PathBuf,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
}
pub(super) struct Policy {
    agents: Vec<AgentConfig>,
    forbidden: Vec<PathBuf>,
}
impl Policy {
    pub fn new(agents: Vec<AgentConfig>, forbidden: Vec<PathBuf>) -> io::Result<Self> {
        let mut ids = HashSet::new();
        if agents.len() > 32
            || agents.iter().any(|agent| {
                !queue_selection::valid(agent.queue_selection.as_ref())
                    || !label(&agent.id)
                    || !label(&agent.label)
                    || !label(&agent.default_mode)
                    || agent.default_mode.trim() != agent.default_mode
                    || !ids.insert(&agent.id)
                    || !agent.executable.is_absolute()
                    || agent.environment.len() > 128
                    || agent.environment.iter().any(|(key, value)| {
                        key.is_empty()
                            || key.len() > 256
                            || key.contains(['=', '\0'])
                            || value.len() > 32_768
                            || value.contains('\0')
                    })
            })
        {
            return Err(io::Error::other("Invalid native agent installation"));
        }
        let mut participants = HashSet::new();
        if agents
            .iter()
            .map(|a| a.participant_selections.len())
            .sum::<usize>()
            > lapis_app_protocol::participants::PARTICIPANT_PROFILE_LIMIT
            || agents
                .iter()
                .flat_map(|a| &a.participant_selections)
                .any(|p| {
                    !participants.insert(p.recipient.id.clone()) || !queue_selection::valid(Some(p))
                })
        {
            return Err(io::Error::other(
                "Invalid declared native participant profiles",
            ));
        }
        let policy = Self { agents, forbidden };
        for agent in &policy.agents {
            policy.executable(agent)?;
        }
        Ok(policy)
    }
    fn executable(&self, agent: &AgentConfig) -> io::Result<PathBuf> {
        if !std::fs::symlink_metadata(&agent.executable)?.is_file() {
            return Err(io::Error::other("Agent helper must be a physical file"));
        }
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(&agent.executable)?.permissions().mode() & 0o111 == 0 {
            return Err(io::Error::other("Agent helper is not executable"));
        }
        let executable = agent.executable.canonicalize()?;
        if self
            .forbidden
            .iter()
            .any(|root| executable.starts_with(root))
        {
            return Err(io::Error::other(
                "Agent helpers must remain outside vault content",
            ));
        }
        Ok(executable)
    }
}
impl AgentPolicy for Policy {
    fn targets(&self, _: Uuid, scope: &VaultRoot) -> lapis_ai_service::Result<Vec<TurnTarget>> {
        scope
            .verify_physical_root()
            .map_err(|_| lapis_ai_service::Error("scope_unavailable"))?;
        Ok(self
            .agents
            .iter()
            .map(|agent| TurnTarget {
                queue_selection: agent.queue_selection.clone(),
                id: agent.id.clone(),
                label: agent.label.clone(),
                default_mode: agent.default_mode.clone(),
            })
            .collect())
    }
    fn participant_profiles(
        &self,
        _: Uuid,
        scope: &VaultRoot,
    ) -> lapis_ai_service::Result<Vec<(String, lapis_app_protocol::queues::QueueSelection)>> {
        scope
            .verify_physical_root()
            .map_err(|_| lapis_ai_service::Error("scope_unavailable"))?;
        Ok(self
            .agents
            .iter()
            .flat_map(|a| {
                a.participant_selections
                    .iter()
                    .map(|p| (a.id.clone(), p.clone()))
            })
            .collect())
    }
    fn resolve(
        &self,
        _: Uuid,
        scope: &VaultRoot,
        target: &str,
    ) -> lapis_ai_service::Result<Launch> {
        let agent = self
            .agents
            .iter()
            .find(|agent| agent.id == target)
            .ok_or(lapis_ai_service::Error("target_unavailable"))?;
        let executable = self
            .executable(agent)
            .map_err(|_| lapis_ai_service::Error("helper_unavailable"))?;
        let mut environment = Environment::default();
        environment.0.extend(agent.environment.clone());
        Ok(Launch {
            prepared: PreparedLaunch {
                executable,
                arguments: vec![],
                directory: scope.path().into(),
                environment,
            },
            directory: scope.path().into(),
            resume: None,
        })
    }
}
fn label(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
