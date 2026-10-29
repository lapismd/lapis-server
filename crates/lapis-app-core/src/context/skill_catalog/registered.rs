use super::{
    discovery::{Buckets, diagnostic, escapes},
    *,
};

pub(super) fn collect(
    sources: &SkillSources,
    context: &SkillContext,
    reader: &mut dyn SkillReader,
    buckets: &mut Buckets,
    diagnostics: &mut Vec<SkillDiagnostic>,
) -> SkillResult<()> {
    for registered in &sources.registered {
        let owner = &registered.owner_plugin_id;
        let source = if registered.kind == "programmatic" {
            SkillSource::Programmatic
        } else {
            SkillSource::Extension
        };
        if context
            .enabled_plugin_ids
            .as_ref()
            .is_some_and(|ids| !ids.is_empty() && !ids.contains(owner))
        {
            let path = registered
                .path
                .as_deref()
                .or_else(|| registered.skill.as_ref().map(|s| s.name.as_str()))
                .unwrap_or(owner);
            diagnostics.push(diagnostic(
                path,
                source,
                &format!("Disabled extension root is excluded: {owner}"),
            ));
            continue;
        }
        if registered.kind == "programmatic"
            && let Some(skill) = &registered.skill
        {
            let loaded = programmatic(owner, skill)?;
            buckets.entry(loaded.source).or_default().push(loaded);
            continue;
        }
        let key = format!("{owner}:{}", registered.path.as_deref().unwrap_or_default());
        let files =
            if let Some((_, files)) = sources.extension_files.iter().find(|(k, _)| k == &key) {
                files.clone()
            } else {
                extension_files(registered, sources, reader)?
            };
        for (relative, content) in files {
            if !relative.ends_with("SKILL.md") {
                continue;
            }
            let path = format!("{owner}/{relative}");
            let root = format!(
                "{owner}/{}",
                relative.strip_suffix("/SKILL.md").unwrap_or(&relative)
            );
            discovery::collect(
                buckets,
                diagnostics,
                &content,
                SkillParseInput {
                    path,
                    source: SkillSource::Extension,
                    root,
                },
            );
        }
    }
    Ok(())
}
fn extension_files(
    source: &RegisteredSkill,
    sources: &SkillSources,
    reader: &mut dyn SkillReader,
) -> SkillResult<Vec<(String, String)>> {
    let Some(path) = &source.path else {
        return Ok(Vec::new());
    };
    let Some((_, root)) = sources
        .extension_roots
        .iter()
        .find(|(owner, _)| owner == &source.owner_plugin_id)
    else {
        return Ok(Vec::new());
    };
    let prefix = format!("{}/{path}", root.strip_suffix('/').unwrap_or(root));
    let mut contents = Vec::new();
    for path in reader.list(&prefix)? {
        let root = path.strip_suffix("/SKILL.md").unwrap_or(&path);
        if !escapes(root, &prefix) {
            let text = reader.read(&path)?;
            contents.push((path[prefix.len() + 1..].into(), text));
        }
    }
    Ok(contents)
}
fn programmatic(owner: &str, skill: &ProgrammaticSkill) -> SkillResult<LoadedSkill> {
    let mut lines = vec![
        "---".into(),
        format!("name: {}", skill.name),
        format!("description: {}", skill.description),
    ];
    if skill.user_invocable == Some(false) {
        lines.push("user-invocable: false".into());
    }
    if skill.disable_model_invocation == Some(true) {
        lines.push("disable-model-invocation: true".into());
    }
    if let Some(hint) = &skill.argument_hint
        && !hint.is_empty()
    {
        lines.push(format!(
            "argument-hint: {}",
            serde_json::to_string(hint).expect("string JSON")
        ));
    }
    if let Some(SkillCommand::Tool { tool, .. }) = &skill.command {
        lines.push("command-dispatch: tool".into());
        lines.push(format!("command-tool: {tool}"));
    }
    lines.extend(["---".into(), String::new(), skill.instructions.clone()]);
    let root = format!("programmatic:{owner}/{}", skill.name);
    parse_skill(
        &lines.join("\n"),
        &SkillParseInput {
            path: root.clone(),
            source: SkillSource::Programmatic,
            root,
        },
    )
}
