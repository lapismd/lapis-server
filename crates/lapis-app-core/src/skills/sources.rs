//! Native package discovery never loads plugin JavaScript.
use super::*;
use crate::plugins::assets;
use serde_json::Value;

pub fn discover(root: &VaultRoot, bundled: &[PackagedSkill]) -> io::Result<Vec<PackagedSkill>> {
    let mut bundles = bundled.to_vec();
    let disabled: Value = if root.stat(".lapis/core-plugins.json")?.is_some() {
        serde_json::from_str(&root.read_text(".lapis/core-plugins.json")?)
            .map_err(|_| invalid("Invalid core plugin enablement"))?
    } else {
        serde_json::json!([])
    };
    let disabled = disabled
        .as_array()
        .or_else(|| disabled["disabled"].as_array())
        .ok_or_else(|| invalid("Invalid core plugin enablement"))?;
    bundles.retain(|bundle| {
        !disabled
            .iter()
            .any(|value| value.as_str() == Some(&bundle.source))
    });
    if root.stat(".lapis/installed-plugins.json")?.is_none() {
        return Ok(bundles);
    }
    let state: Value = serde_json::from_str(&root.read_text(".lapis/installed-plugins.json")?)
        .map_err(|_| invalid("Invalid plugin installation state"))?;
    if state["schemaVersion"] != 1 {
        return Err(invalid("Invalid plugin installation state"));
    }
    let configured: Vec<String> = if root.stat(".lapis/community-plugins.json")?.is_some() {
        serde_json::from_str(&root.read_text(".lapis/community-plugins.json")?)
            .map_err(|_| invalid("Invalid plugin enablement"))?
    } else {
        Vec::new()
    };
    let records = state["plugins"]
        .as_object()
        .ok_or_else(|| invalid("Invalid plugin installation state"))?;
    for id in &configured {
        if disabled.iter().any(|value| value.as_str() == Some(id)) {
            continue;
        }
        let Some(record) = records.get(id) else {
            continue;
        };
        if record.get("revoked").is_some_and(|value| !value.is_null()) {
            continue;
        }
        let version = record["installedVersion"]
            .as_str()
            .ok_or_else(|| invalid("Invalid plugin version"))?;
        let fingerprint = assets::installation_fingerprint(root, id)?;
        let read = |file: &str| -> io::Result<String> {
            String::from_utf8(assets::read(root, id, version, &fingerprint, file)?.bytes)
                .map_err(|_| invalid("Plugin skill is not UTF-8"))
        };
        let manifest: Value = serde_json::from_str(&read("manifest.json")?)
            .map_err(|_| invalid("Invalid plugin manifest"))?;
        if manifest["id"] != *id || manifest["version"] != version {
            return Err(invalid("Plugin manifest identity changed"));
        }
        let Some(skills) = manifest["lapis"]["contributes"]["agentSkills"].as_array() else {
            continue;
        };
        let files = record["files"]
            .as_array()
            .ok_or_else(|| invalid("Invalid plugin file list"))?;
        for skill in skills {
            let name = skill["name"]
                .as_str()
                .ok_or_else(|| invalid("Invalid skill name"))?;
            let directory = skill["directory"]
                .as_str()
                .filter(|value| path(value))
                .ok_or_else(|| invalid("Invalid skill directory"))?;
            let prefix = format!("{directory}/");
            let mut contents = BTreeMap::new();
            for entry in files {
                let file = entry["path"]
                    .as_str()
                    .ok_or_else(|| invalid("Invalid plugin asset"))?;
                if let Some(relative) = file.strip_prefix(&prefix) {
                    contents.insert(relative.into(), read(file)?);
                }
            }
            bundles.push(PackagedSkill {
                name: name.into(),
                source: id.clone(),
                version: version.into(),
                files: contents,
            });
        }
    }
    bundles.retain(|bundle| {
        !disabled
            .iter()
            .any(|value| value.as_str() == Some(&bundle.source))
    });
    Ok(bundles)
}
