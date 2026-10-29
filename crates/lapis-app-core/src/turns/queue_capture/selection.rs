use super::{Result, fields::*};
use serde_json::Value;
use std::collections::HashSet;

pub(crate) fn recipients(value: Option<&Value>) -> Result<Vec<String>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Some(list) = value.as_array().filter(|v| v.len() <= 4) else {
        return fail("Message recipients must contain at most four agents");
    };
    let mut seen = HashSet::new();
    let mut ids = Vec::new();
    for entry in list {
        let Some(o) = entry.as_object() else {
            return fail("Message recipient is invalid");
        };
        let valid = || -> Option<&str> {
            if o.keys()
                .any(|k| !["id", "token", "label"].contains(&k.as_str()))
            {
                return None;
            }
            let id = entry["id"].as_str()?;
            let token = entry["token"].as_str()?;
            let label = entry["label"].as_str()?;
            let hex = token.strip_prefix("jj-store-agent:")?.strip_suffix(';')?;
            if !(2..=512).contains(&hex.len())
                || hex.len() % 2 != 0
                || !hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return None;
            }
            let bytes = hex
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
                .collect::<Option<Vec<_>>>()?;
            let decoded = std::str::from_utf8(&bytes).ok()?;
            if decoded != id
                || id.is_empty()
                || utf16(id) > 64
                || id.starts_with('\u{feff}')
                || id.chars().any(|c| c <= '\u{1f}' || c == '\u{7f}')
                || crate::text::trim(label).is_empty()
                || label.chars().count() > 80
                || label
                    .chars()
                    .any(|c| c <= '\u{1f}' || matches!(c, '\u{7f}' | '\u{2028}' | '\u{2029}'))
            {
                return None;
            }
            Some(id)
        };
        let Some(id) = valid() else {
            return fail("Message recipient identity or label is invalid");
        };
        if !seen.insert(id) {
            return fail("Message recipient identity or label is invalid");
        }
        ids.push(id.to_owned());
    }
    Ok(ids)
}
pub(super) fn mode(value: Option<&Value>) -> Result<()> {
    if value.is_some_and(|v| {
        v.as_str().is_none_or(|s| {
            crate::text::trim(s).is_empty()
                || utf16(s) > 256
                || s.chars().any(|c| c <= '\u{1f}' || c == '\u{7f}')
        })
    }) {
        return fail("Invalid agent execution mode");
    }
    Ok(())
}
fn profile(value: Option<&Value>) -> Result<()> {
    let Some(value) = value else { return Ok(()) };
    let Some(o) = value.as_object() else {
        return fail("Invalid agent context");
    };
    if o.keys().any(|k| {
        !["name", "description", "instructions", "providerProfileId"].contains(&k.as_str())
    }) {
        return fail("Invalid agent context field");
    }
    if value.get("providerProfileId").is_some_and(|v| {
        v.as_str().is_none_or(|s| {
            s.is_empty()
                || s.len() > 128
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
    }) {
        return fail("Invalid provider configuration identity");
    }
    for (key, limit) in [("name", 80), ("description", 280), ("instructions", 32768)] {
        if value.get(key).is_some_and(|v| {
            v.as_str().is_none_or(|s| {
                s.contains('\0')
                    || if key == "instructions" {
                        s.len() > limit
                    } else {
                        s.chars().count() > limit
                    }
            })
        }) {
            return fail(format!("Invalid agent {key}"));
        }
    }
    Ok(())
}
pub(super) fn selection(value: &Value, ids: &[String]) -> Result<()> {
    record(value, "Queue selection")?;
    fields(value, &["targets", "runtime", "thinking", "attachments"])?;
    if !one_of(
        &value["runtime"],
        &["acp", "codex-native", "opencode-native", "fake"],
    ) {
        return fail("Invalid queued runtime preference");
    }
    thinking(value.get("thinking"), "Queue thinking")?;
    let Some(targets) = value["targets"].as_array().filter(|v| v.len() == ids.len()) else {
        return fail("Queue targets do not match recipients");
    };
    for (target, id) in targets.iter().zip(ids) {
        record(target, "Queue target")?;
        fields(
            target,
            &[
                "id",
                "agent",
                "model",
                "thinking",
                "executionMode",
                "agentProfile",
            ],
        )?;
        mode(target.get("executionMode"))?;
        profile(target.get("agentProfile"))?;
        thinking(target.get("thinking"), "Queued target thinking")?;
        if target["id"] != id.as_str()
            || !["codex", "cursor", "opencode", "local"]
                .iter()
                .any(|a| target["agent"] == *a)
        {
            return fail("Invalid queued target");
        }
        if let Some(model) = target.get("model") {
            record(model, "Queue model")?;
            fields(model, &["provider", "model"])?;
            for key in ["provider", "model"] {
                if utf16(string(model, key, "Queue model")?) > 512 {
                    return fail("Queue model is too long");
                }
            }
        }
    }
    let Some(paths) = value["attachments"].as_array().filter(|a| a.len() <= 8) else {
        return fail("Queue attachment limit exceeded");
    };
    for path in paths {
        let Some(path) = path.as_str().filter(|s| utf16(s) <= 2048) else {
            return fail("Invalid queued file path");
        };
        let raw = crate::text::trim(path).replace('\\', "/");
        if matches!(raw.as_str(), "" | "." | "/") {
            return fail("Queued attachment must not be the vault root");
        }
        if raw.starts_with('/')
            || (raw.len() >= 3
                && raw.as_bytes()[0].is_ascii_alphabetic()
                && raw.as_bytes()[1] == b':'
                && raw.as_bytes()[2] == b'/')
            || raw.contains("://")
        {
            return fail("Queued attachment must be vault-relative");
        }
        let raw = raw.strip_prefix("./").unwrap_or(&raw).trim_end_matches('/');
        if raw.split('/').any(|s| matches!(s, "" | "." | "..")) {
            return fail("Queued attachment must be a confined vault-relative path");
        }
    }
    Ok(())
}
