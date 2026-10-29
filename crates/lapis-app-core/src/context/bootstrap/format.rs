// SPDX-License-Identifier: Apache-2.0
use super::{AgentBootstrap, invalid, rules::has_host_path};
use crate::services::Result;

pub(super) fn render(bootstrap: &AgentBootstrap) -> Result<String> {
    let tools = if bootstrap.tools.is_empty() {
        "(none)".into()
    } else {
        bootstrap
            .tools
            .iter()
            .map(|tool| format!("- {} — {}", tool.name, one_line(&tool.description)))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let skills = if bootstrap.skills.is_empty() {
        "(none)".into()
    } else {
        bootstrap
            .skills
            .iter()
            .map(|skill| format!("- {}", skill.name))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let folder = bootstrap
        .folder_instructions
        .iter()
        .filter(|entry| entry.omitted.is_none() && !crate::text::trim(&entry.text).is_empty())
        .map(|entry| format!("--- {} ---\n{}", entry.path, entry.text))
        .collect::<Vec<_>>()
        .join("\n\n");
    let workspace = bootstrap
        .workspace
        .as_ref()
        .map(|w| crate::text::trim(&w.label))
        .filter(|label| !label.is_empty())
        .unwrap_or("(none)");
    let mut body = vec![
        "<lapis_context>".into(),
        bootstrap.instructions.join("\n\n"),
        String::new(),
        format!(
            "Current scope: {}",
            portable(Some(&bootstrap.conversation.scope_dir), "(vault root)")
        ),
        format!(
            "Launched from: {}",
            portable(bootstrap.conversation.launch_note_path.as_deref(), "(none)")
        ),
        format!("Workspace: {workspace}"),
        String::new(),
        "Available application tools:".into(),
        tools,
        String::new(),
        "Available skills:".into(),
        skills,
    ];
    if !folder.is_empty() {
        body.extend([String::new(), "Folder instructions:".into(), folder]);
    }
    body.push("</lapis_context>".into());
    let text = body.join("\n");
    if has_host_path(&text) {
        return Err(invalid(
            "Session bootstrap must not include host filesystem paths.",
        ));
    }
    Ok(text)
}

fn portable(value: Option<&str>, empty: &str) -> String {
    let value = crate::text::trim(value.unwrap_or_default());
    if value.is_empty() {
        empty.into()
    } else if has_host_path(value) {
        "(application folder)".into()
    } else {
        value.into()
    }
}

fn one_line(value: &str) -> String {
    value
        .split(|c: char| {
            let mut bytes = [0u8; 4];
            crate::text::trim(c.encode_utf8(&mut bytes)).is_empty()
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
