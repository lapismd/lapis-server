use super::*;
use crate::context::skill_catalog::SkillReader;

pub(super) struct Reader<'a, F> {
    pub work: &'a Work,
    pub cancelled: &'a F,
}
impl<F: Fn() -> bool> SkillReader for Reader<'_, F> {
    fn list(&mut self, prefix: &str) -> Result<Vec<String>> {
        self.work.check_skills(self.cancelled)?;
        let mut pending = vec![prefix.to_owned()];
        let mut files = Vec::new();
        while let Some(path) = pending.pop() {
            self.work.check_skills(self.cancelled)?;
            let listed = {
                let state = self.work.vault.lock()?;
                let result = state.root.list(&path);
                state.root.verify_physical_root()?;
                match result {
                    Ok(listed) => listed,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(error.into()),
                }
            };
            self.work.check_skills(self.cancelled)?;
            for file in listed.files {
                if file == "SKILL.md" {
                    files.push(format!("{path}/{file}"));
                }
            }
            for folder in listed.folders.into_iter().rev() {
                pending.push(format!("{path}/{folder}"));
            }
        }
        self.work.check_skills(self.cancelled)?;
        Ok(files)
    }
    fn read(&mut self, path: &str) -> Result<String> {
        self.work.check_skills(self.cancelled)?;
        let value = self.work.read_text(path);
        self.work.check_skills(self.cancelled)?;
        Ok(value?.text)
    }
}
