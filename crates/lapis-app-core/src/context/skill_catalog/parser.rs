use super::*;
use regress::Regex;
use serde_yaml::{Mapping, Value};
use std::sync::OnceLock;

pub fn skill_content_version(content: &str) -> SkillResult<String> {
    let hash = content.encode_utf16().fold(2_166_136_261u32, |hash, unit| {
        (hash ^ u32::from(unit)).wrapping_mul(16_777_619)
    });
    Ok(format!("fnv1a:{hash:08x}"))
}
pub fn parse_skill(content: &str, input: &SkillParseInput) -> SkillResult<LoadedSkill> {
    static FRONTMATTER: OnceLock<Regex> = OnceLock::new();
    let regex = FRONTMATTER.get_or_init(|| {
        Regex::with_flags(r"^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$", "u")
            .expect("retained skill frontmatter expression")
    });
    let fail = |message: String| SkillFailure {
        name: "SkillParseError".into(),
        path: Some(input.path.clone()),
        ..failure(message)
    };
    let found = regex
        .find(content)
        .ok_or_else(|| fail("SKILL.md must start with YAML frontmatter.".into()))?;
    let yaml = &content[found.group(1).expect("frontmatter capture")];
    let data: Value = serde_yaml::from_str(yaml)
        .map_err(|_| fail("SKILL.md frontmatter is not valid YAML.".into()))?;
    let data = untag(&data)
        .as_mapping()
        .ok_or_else(|| fail("SKILL.md frontmatter must be a mapping.".into()))?;
    let name =
        string(field(data, "name")).ok_or_else(|| fail("SKILL.md must declare name.".into()))?;
    if !valid_name(&name) {
        return Err(fail(format!("Invalid skill name: {name}")));
    }
    let description = string(field(data, "description"))
        .ok_or_else(|| fail("SKILL.md must declare description.".into()))?;
    let command = command(data).map_err(fail)?;
    Ok(LoadedSkill {
        id: format!("{}:{name}", input.source.name()),
        name,
        description,
        source: input.source,
        root: input.root.clone(),
        version: skill_content_version(content)?,
        user_invocable: boolean(field(data, "user-invocable"), true),
        model_invocable: !boolean(field(data, "disable-model-invocation"), false),
        argument_hint: string(field(data, "argument-hint")),
        command,
        requirements: requirements(data),
        instructions: crate::text::trim(&content[found.group(2).expect("body capture")]).into(),
    })
}
fn valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
}
fn untag(value: &Value) -> &Value {
    match value {
        Value::Tagged(tagged) => untag(&tagged.value),
        _ => value,
    }
}
fn field<'a>(data: &'a Mapping, name: &str) -> Option<&'a Value> {
    data.get(Value::String(name.into())).map(untag)
}
fn string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(crate::text::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}
fn boolean(value: Option<&Value>, fallback: bool) -> bool {
    value.and_then(Value::as_bool).unwrap_or(fallback)
}
fn list(value: Option<&Value>) -> Option<Vec<String>> {
    let items = value?
        .as_sequence()?
        .iter()
        .filter_map(|v| string(Some(untag(v))))
        .collect::<Vec<_>>();
    (!items.is_empty()).then_some(items)
}
fn command(data: &Mapping) -> Result<SkillCommand, String> {
    let dispatch = string(field(data, "command-dispatch"));
    match dispatch.as_deref() {
        None | Some("model") => Ok(SkillCommand::Model),
        Some("tool") => {
            let tool = string(field(data, "command-tool"))
                .ok_or("command-dispatch: tool requires command-tool")?;
            let arg_mode = string(field(data, "command-arg-mode")).unwrap_or("raw".into());
            if arg_mode != "raw" {
                return Err(format!("Unsupported command-arg-mode: {arg_mode}"));
            }
            Ok(SkillCommand::Tool { tool, arg_mode })
        }
        Some(other) => Err(format!("Unsupported command-dispatch: {other}")),
    }
}
fn requirements(data: &Mapping) -> Option<SkillRequirements> {
    let metadata = field(data, "metadata")?.as_mapping()?;
    let lapis = field(metadata, "lapis")?.as_mapping()?;
    let requires = field(lapis, "requires")?.as_mapping()?;
    let result = SkillRequirements {
        tools: list(field(requires, "tools")),
        capabilities: list(field(requires, "capabilities")),
        extensions: list(field(requires, "extensions")),
    };
    (result.tools.is_some() || result.capabilities.is_some() || result.extensions.is_some())
        .then_some(result)
}
