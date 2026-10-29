mod reader;
use super::{Error, Result, Work};
use crate::context::skill_catalog::{
    LoadedSkill, SkillContext, SkillRegistry, SkillSnapshot, SkillSources, discover_skills,
};
use lapis_app_protocol::ErrorCode;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Default)]
pub(super) struct State {
    registry: SkillRegistry,
    bindings: HashMap<Uuid, Binding>,
}
struct Binding {
    snapshot: SkillSnapshot,
    loaded: Vec<LoadedSkill>,
}

impl Work {
    /// Capture one binding from trusted native sources; this grants no execution rights.
    pub fn capture_skill_binding(
        &self,
        binding: Uuid,
        context: &SkillContext,
        sources: &SkillSources,
        cancelled: impl Fn() -> bool,
    ) -> Result<SkillSnapshot> {
        self.check_skills(&cancelled)?;
        {
            let state = self.vault.lock()?;
            state.require_open()?;
            state.root.verify_physical_root()?;
            if !context.scope_dir.is_empty() {
                state.root.open_child(&context.scope_dir)?;
            }
            let skills = self.vault.skills.lock().map_err(|_| Error::closed())?;
            if let Some(bound) = skills.bindings.get(&binding) {
                return Ok(bound.snapshot.clone());
            }
        }
        let prepared = discover_skills(
            context,
            sources,
            &mut reader::Reader {
                work: self,
                cancelled: &cancelled,
            },
        )?;
        self.check_skills(&cancelled)?;
        let state = self.vault.lock()?;
        state.require_open()?;
        state.root.verify_physical_root()?;
        let mut skills = self.vault.skills.lock().map_err(|_| Error::closed())?;
        if let Some(bound) = skills.bindings.get(&binding) {
            return Ok(bound.snapshot.clone());
        }
        let snapshot = skills.registry.capture_discovered(
            prepared,
            &Uuid::new_v4().to_string(),
            &chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        );
        let loaded = snapshot
            .skills
            .iter()
            .map(|entry| {
                skills
                    .registry
                    .get_loaded(&entry.name)
                    .expect("captured catalog entry")
                    .clone()
            })
            .collect();
        skills.bindings.insert(
            binding,
            Binding {
                snapshot: snapshot.clone(),
                loaded,
            },
        );
        Ok(snapshot)
    }
    /// Read exact captured bytes rather than rediscovering a later file version.
    pub fn bound_skill(&self, binding: Uuid, name: &str) -> Result<LoadedSkill> {
        let state = self.vault.lock()?;
        state.require_open()?;
        state.root.verify_physical_root()?;
        let skills = self.vault.skills.lock().map_err(|_| Error::closed())?;
        skills
            .bindings
            .get(&binding)
            .and_then(|bound| bound.loaded.iter().find(|s| s.name == name))
            .cloned()
            .ok_or_else(|| Error::new(ErrorCode::NotFound, &format!("Unknown skill: {name}")))
    }
    pub fn release_skill_binding(&self, binding: Uuid) -> Result<()> {
        let state = self.vault.lock()?;
        state.require_open()?;
        state.root.verify_physical_root()?;
        self.vault
            .skills
            .lock()
            .map_err(|_| Error::closed())?
            .bindings
            .remove(&binding);
        Ok(())
    }
    pub fn invalidate_skill_catalog(&self) -> Result<()> {
        let state = self.vault.lock()?;
        state.require_open()?;
        state.root.verify_physical_root()?;
        self.vault
            .skills
            .lock()
            .map_err(|_| Error::closed())?
            .registry
            .invalidate();
        Ok(())
    }
    fn check_skills(&self, cancelled: &impl Fn() -> bool) -> Result<()> {
        if cancelled() || self.cancellation_requested() {
            return Err(Error::new(ErrorCode::Closed, "This operation was aborted"));
        }
        let state = self.vault.lock()?;
        state.require_open()?;
        state.root.verify_physical_root()?;
        Ok(())
    }
}
