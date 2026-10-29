use super::*;

#[derive(Default)]
pub struct SkillRegistry {
    loaded: Vec<LoadedSkill>,
    history: Vec<(String, LoadedSkill)>,
    generation: u64,
    diagnostics: Vec<SkillDiagnostic>,
}
impl SkillRegistry {
    pub fn invalidate(&mut self) {
        self.generation += 1;
        self.loaded.clear();
    }
    pub fn discover(
        &mut self,
        context: &SkillContext,
        sources: &SkillSources,
        reader: &mut dyn SkillReader,
    ) -> SkillResult<Vec<LoadedSkill>> {
        let discovered = discover_skills(context, sources, reader)?;
        self.install(discovered);
        Ok(self.loaded.clone())
    }
    fn install(&mut self, discovered: DiscoveredSkills) {
        self.diagnostics = discovered.diagnostics;
        self.loaded = discovered.skills;
        for skill in &self.loaded {
            let key = format!("{}@{}", skill.id, skill.version);
            if let Some((_, entry)) = self.history.iter_mut().find(|(k, _)| k == &key) {
                *entry = skill.clone();
            } else {
                self.history.push((key, skill.clone()));
            }
        }
    }
    pub fn snapshot(
        &mut self,
        context: &SkillContext,
        sources: &SkillSources,
        reader: &mut dyn SkillReader,
        nonce: &str,
        created_at: &str,
    ) -> SkillResult<SkillSnapshot> {
        self.discover(context, sources, reader)?;
        Ok(self.snapshot_loaded(nonce, created_at))
    }
    pub(crate) fn capture_discovered(
        &mut self,
        discovered: DiscoveredSkills,
        nonce: &str,
        created_at: &str,
    ) -> SkillSnapshot {
        self.install(discovered);
        self.snapshot_loaded(nonce, created_at)
    }
    fn snapshot_loaded(&self, nonce: &str, created_at: &str) -> SkillSnapshot {
        let skills = self
            .loaded
            .iter()
            .map(|skill| SkillSnapshotEntry {
                skill_id: skill.id.clone(),
                name: skill.name.clone(),
                description: skill.description.clone(),
                version: skill.version.clone(),
                user_invocable: skill.user_invocable,
                model_invocable: skill.model_invocable,
                argument_hint: skill.argument_hint.clone(),
            })
            .collect();
        SkillSnapshot {
            id: format!("skill-snapshot-{}-{nonce}", self.generation),
            created_at: created_at.into(),
            skills,
        }
    }
    pub fn load(&self, id: &str, version: &str) -> SkillResult<LoadedSkill> {
        let key = format!("{id}@{version}");
        let loaded = self
            .history
            .iter()
            .find(|(k, _)| k == &key)
            .map(|(_, s)| s)
            .or_else(|| self.loaded.iter().find(|s| s.id == id))
            .or_else(|| {
                self.history
                    .iter()
                    .map(|(_, s)| s)
                    .find(|s| s.version == version && (s.id == id || s.name == id))
            });
        loaded
            .filter(|s| s.version == version)
            .cloned()
            .ok_or_else(|| failure(format!("Skill is not available: {id}")))
    }
    pub fn activate(
        &mut self,
        context: &SkillContext,
        sources: &SkillSources,
        reader: &mut dyn SkillReader,
        request: &SkillActivationRequest,
    ) -> SkillResult<SkillActivation> {
        let descriptor = self
            .discover(context, sources, reader)?
            .into_iter()
            .find(|s| s.name == request.name)
            .ok_or_else(|| failure(format!("Unknown skill: {}", request.name)))?;
        let loaded = self.load(&descriptor.id, &descriptor.version)?;
        Ok(SkillActivation {
            skill_id: loaded.id,
            skill_name: loaded.name,
            version: loaded.version,
            source: request.source.clone(),
            arguments: request.arguments.clone(),
            instructions: loaded.instructions,
        })
    }
    pub fn get_loaded(&self, name: &str) -> Option<&LoadedSkill> {
        self.loaded.iter().find(|s| s.name == name)
    }
    pub fn diagnostics(&self) -> &[SkillDiagnostic] {
        &self.diagnostics
    }
}
