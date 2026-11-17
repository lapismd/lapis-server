use super::{Client, Result, guards};
use crate::skills::{self, PackagedSkill, SkillResult};
impl Client {
    pub fn manage_packaged_skills(
        &self,
        bundled: &[PackagedSkill],
        action: &str,
        selected: Option<&str>,
    ) -> Result<Vec<SkillResult>> {
        let state = self.vault.lock()?;
        state.require_client(self.id)?;
        state.root.verify_physical_root()?;
        if action != "list" {
            guards::write(&state, ".agents", None, true)?;
        }
        let sources = skills::sources::discover(&state.root, bundled)?;
        Ok(skills::manage(&state.root, &sources, action, selected)?)
    }
}
