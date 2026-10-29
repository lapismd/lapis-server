// SPDX-License-Identifier: Apache-2.0
//! Borrowed skill-context projection does not discover or authorize skills.
use crate::services::{Error, Result};
use lapis_app_protocol::ErrorCode;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillManifestEntry {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub model_invocable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SelectedSkillActivation {
    pub skill_id: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
    pub instructions: String,
}

pub fn available_skills_manifest(skills: &[SkillManifestEntry]) -> Result<String> {
    let body = skills
        .iter()
        .filter(|skill| skill.model_invocable)
        .map(|skill| {
            let escape = super::projection::attribute;
            format!(
                "  <skill>\n    <name>{}</name>\n    <description>{}</description>\n    <id>{}</id>\n    <version>{}</version>\n  </skill>",
                escape(&skill.name), escape(&skill.description), escape(&skill.skill_id), escape(&skill.version)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let xml = format!(
        "<available_skills>\n{body}\n</available_skills>\n\nWhen a listed skill clearly applies, load it using skills_read before proceeding.\nOnly load skills listed in available_skills."
    );
    if xml.encode_utf16().count() > 8_000 {
        return Err(Error::new(
            ErrorCode::InvalidRequest,
            "Available-skills manifest exceeds 8000 characters.",
        ));
    }
    Ok(xml)
}
