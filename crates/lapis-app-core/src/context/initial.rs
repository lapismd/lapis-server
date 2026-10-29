// SPDX-License-Identifier: Apache-2.0
use super::{SelectedSkillActivation, SkillManifestEntry, available_skills_manifest};
use crate::services::{Error, Result};
use lapis_app_protocol::ErrorCode;

pub fn prepare_initial_context(
    bootstrap: Option<&str>,
    skills: Option<&[SkillManifestEntry]>,
    activations: &[SelectedSkillActivation],
) -> Result<Option<String>> {
    let mut parts = Vec::new();
    if let Some(bootstrap) = bootstrap {
        if bootstrap.encode_utf16().count() > 65_536 {
            return Err(invalid("Invalid AI session bootstrap"));
        }
        if !bootstrap.is_empty() {
            parts.push(bootstrap.to_owned());
        }
    }
    if let Some(skills) = skills {
        if skills.len() > 128 {
            return Err(invalid("Invalid AI skill snapshot"));
        }
        parts.push(available_skills_manifest(skills)?);
    }
    if activations.len() > 16 {
        return Err(invalid("Invalid AI skill activations"));
    }
    for activation in activations {
        if activation.instructions.encode_utf16().count() > 65_536 {
            return Err(invalid("Invalid AI skill activation"));
        }
        // Field declaration order matches the source's explicitly selected JSON.
        let text = serde_json::to_string(activation)
            .map_err(|_| invalid("Invalid AI skill activation"))?
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        parts.push(format!(
            "Application-selected skill instructions; application permissions still apply.\n<skill-activation>\n{text}\n</skill-activation>"
        ));
    }
    let context = parts.join("\n\n");
    if context.len() > 131_072 {
        return Err(invalid("AI initial context exceeds its limit"));
    }
    Ok((!context.is_empty()).then_some(context))
}

fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InvalidRequest, message)
}
