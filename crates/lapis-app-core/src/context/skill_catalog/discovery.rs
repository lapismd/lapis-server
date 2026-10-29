use super::*;
use std::collections::HashMap;
pub(super) type Buckets = HashMap<SkillSource, Vec<LoadedSkill>>;

pub fn discover_skills(
    context: &SkillContext,
    sources: &SkillSources,
    reader: &mut dyn SkillReader,
) -> SkillResult<DiscoveredSkills> {
    let mut result = DiscoveredSkills::default();
    let mut buckets = Buckets::new();
    let folder = if context.scope_dir.is_empty() {
        ".agents/skills".into()
    } else {
        format!("{}/.agents/skills", context.scope_dir)
    };
    for (source, prefix) in [
        (SkillSource::Folder, folder.as_str()),
        (SkillSource::Vault, ".agents/skills"),
        (
            SkillSource::User,
            sources
                .user_prefix
                .as_deref()
                .unwrap_or(".agents/user/skills"),
        ),
    ] {
        let root = prefix.strip_suffix('/').unwrap_or(prefix);
        for path in reader.list(if root.is_empty() {
            ".agents/skills"
        } else {
            root
        })? {
            if path.contains("/scripts/") || path.contains("/references/") {
                continue;
            }
            let root = path.strip_suffix("/SKILL.md").unwrap_or(&path);
            if escapes(root, prefix) {
                result.diagnostics.push(diagnostic(
                    &path,
                    source,
                    "Skill path escapes the trusted root.",
                ));
                continue;
            }
            let content = reader.read(&path)?;
            collect(
                &mut buckets,
                &mut result.diagnostics,
                &content,
                SkillParseInput {
                    path: path.clone(),
                    source,
                    root: root.into(),
                },
            );
        }
    }
    registered::collect(
        sources,
        context,
        reader,
        &mut buckets,
        &mut result.diagnostics,
    )?;
    for skill in &sources.bundled {
        buckets.entry(skill.source).or_default().push(skill.clone());
    }
    result.skills = precedence::merge(buckets, &mut result.diagnostics, context);
    Ok(result)
}
pub(super) fn collect(
    buckets: &mut Buckets,
    diagnostics: &mut Vec<SkillDiagnostic>,
    content: &str,
    input: SkillParseInput,
) {
    match parse_skill(content, &input) {
        Ok(skill) => buckets.entry(skill.source).or_default().push(skill),
        Err(error) => diagnostics.push(diagnostic(&input.path, input.source, &error.message)),
    }
}
pub(super) fn diagnostic(path: &str, source: SkillSource, message: &str) -> SkillDiagnostic {
    SkillDiagnostic {
        path: path.into(),
        source,
        message: message.into(),
        shadowed_by: None,
    }
}
pub(super) fn escapes(root: &str, prefix: &str) -> bool {
    let prefix = prefix.strip_suffix('/').unwrap_or(prefix);
    root.contains("/../")
        || root.ends_with("/..")
        || root.contains("/./")
        || (!prefix.is_empty() && root != prefix && !root.starts_with(&format!("{prefix}/")))
}
