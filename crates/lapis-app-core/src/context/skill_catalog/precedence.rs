use super::{
    discovery::{Buckets, diagnostic},
    *,
};
use icu_collator::CollatorBorrowed;
use std::collections::HashSet;

pub(super) fn merge(
    mut buckets: Buckets,
    diagnostics: &mut Vec<SkillDiagnostic>,
    context: &SkillContext,
) -> Vec<LoadedSkill> {
    let mut winners = Vec::<LoadedSkill>::new();
    for source in [
        SkillSource::Bundled,
        SkillSource::Extension,
        SkillSource::Programmatic,
        SkillSource::User,
        SkillSource::Vault,
        SkillSource::Folder,
    ] {
        let mut seen = HashSet::new();
        for skill in buckets.remove(&source).unwrap_or_default() {
            if !seen.insert(skill.name.clone()) {
                diagnostics.push(diagnostic(
                    &skill.root,
                    skill.source,
                    &format!(
                        "Duplicate skill name at the same precedence: {}",
                        skill.name
                    ),
                ));
                winners.retain(|s| s.name != skill.name);
                continue;
            }
            if let Some(reason) = ineligible(&skill, context) {
                diagnostics.push(diagnostic(&skill.root, skill.source, &reason));
                continue;
            }
            if let Some(index) = winners.iter().position(|s| s.name == skill.name) {
                let existing = &winners[index];
                if existing.source.rank() > skill.source.rank() {
                    continue;
                }
                let mut shadow = diagnostic(
                    &existing.root,
                    existing.source,
                    &format!("Shadowed by {} skill {}", skill.source.name(), skill.name),
                );
                shadow.shadowed_by = Some(skill.root.clone());
                diagnostics.push(shadow);
                winners[index] = skill;
            } else {
                winners.push(skill);
            }
        }
    }
    let collator = CollatorBorrowed::try_new(Default::default(), Default::default())
        .expect("retained skill name collation");
    winners.sort_by(|a, b| collator.compare(&a.name, &b.name));
    winners
}
fn ineligible(skill: &LoadedSkill, context: &SkillContext) -> Option<String> {
    let required = skill.requirements.as_ref()?;
    for tool in required.tools.iter().flatten() {
        if !context
            .available_tool_names
            .as_ref()
            .is_some_and(|tools| tools.contains(tool))
        {
            return Some(format!("Required tool is unavailable: {tool}"));
        }
    }
    if let Some(enabled) = &context.enabled_plugin_ids
        && !enabled.is_empty()
    {
        for id in required.extensions.iter().flatten() {
            if !enabled.contains(id) {
                return Some(format!("Required extension is unavailable: {id}"));
            }
        }
    }
    None
}
impl SkillSource {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::Vault => "vault",
            Self::User => "user",
            Self::Extension => "extension",
            Self::Programmatic => "programmatic",
            Self::Bundled => "bundled",
        }
    }
    fn rank(self) -> u8 {
        match self {
            Self::Folder => 5,
            Self::Vault => 4,
            Self::User => 3,
            Self::Extension | Self::Programmatic => 2,
            Self::Bundled => 1,
        }
    }
}
