use super::*;
use crate::{plugins::assets, vault::VaultRoot};
use serde_json::Value;
use std::io::Read;

pub(super) struct InstalledJob {
    pub plugin: String,
    pub fingerprint: String,
    pub descriptor: JobDescriptor,
}

fn read_json(root: &VaultRoot, path: &str) -> Result<Value> {
    let mut bytes = Vec::new();
    root.open_file(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err(invalid("Plugin configuration exceeds bounds"));
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid("Invalid plugin configuration"))
}

pub(super) fn discover(vault: &VaultService) -> Result<Vec<InstalledJob>> {
    let state = vault.lock()?;
    state.require_open()?;
    let root = &state.root;
    if root.stat(".lapis/installed-plugins.json")?.is_none() {
        return Ok(Vec::new());
    }
    let installed = read_json(root, ".lapis/installed-plugins.json")?;
    if installed["schemaVersion"] != 1 {
        return Err(invalid("Invalid installed plugin state"));
    }
    let records = installed["plugins"]
        .as_object()
        .ok_or_else(|| invalid("Invalid installed plugins"))?;
    let enabled = if root.stat(".lapis/community-plugins.json")?.is_some() {
        read_json(root, ".lapis/community-plugins.json")?
    } else {
        serde_json::json!([])
    };
    let enabled = enabled
        .as_array()
        .ok_or_else(|| invalid("Invalid plugin enablement"))?;
    let disabled = if root.stat(".lapis/core-plugins.json")?.is_some() {
        read_json(root, ".lapis/core-plugins.json")?
    } else {
        serde_json::json!([])
    };
    let disabled = disabled
        .as_array()
        .or_else(|| disabled["disabled"].as_array())
        .ok_or_else(|| invalid("Invalid core plugin enablement"))?;
    let mut output = Vec::new();
    for (plugin, record) in records {
        if !enabled.iter().any(|id| id.as_str() == Some(plugin))
            || disabled.iter().any(|id| id.as_str() == Some(plugin))
            || record.get("revoked").is_some_and(|value| !value.is_null())
        {
            continue;
        }
        let version = record["installedVersion"]
            .as_str()
            .ok_or_else(|| invalid("Invalid plugin version"))?;
        let fingerprint = assets::installation_fingerprint(root, plugin)?;
        let manifest = assets::read(root, plugin, version, &fingerprint, "manifest.json")?;
        if manifest.bytes.len() > 65536 {
            return Err(invalid("Plugin manifest exceeds bounds"));
        }
        let manifest: Value = serde_json::from_slice(&manifest.bytes)
            .map_err(|_| invalid("Invalid plugin manifest"))?;
        if manifest["id"] != *plugin || manifest["version"] != version {
            return Err(invalid("Plugin manifest identity changed"));
        }
        let Some(jobs) = manifest["lapis"]["contributes"].get("backgroundJobs") else {
            continue;
        };
        let jobs: Vec<JobDescriptor> = serde_json::from_value(jobs.clone())
            .map_err(|_| invalid("Invalid background job declarations"))?;
        if jobs.len() > 16 {
            return Err(invalid("Too many background job declarations"));
        }
        let mut identities = std::collections::HashSet::new();
        for descriptor in jobs {
            validate(&descriptor)?;
            if !identities.insert(descriptor.id.clone()) || output.len() >= 64 {
                return Err(invalid("Duplicate or excessive background jobs"));
            }
            output.push(InstalledJob {
                plugin: plugin.clone(),
                fingerprint: fingerprint.clone(),
                descriptor,
            });
        }
    }
    output.sort_by(|a, b| (&a.plugin, &a.descriptor.id).cmp(&(&b.plugin, &b.descriptor.id)));
    Ok(output)
}

pub(super) fn validate(descriptor: &JobDescriptor) -> Result<()> {
    if descriptor.id.is_empty()
        || descriptor.id.len() > 64
        || !descriptor
            .id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || descriptor.title.trim().is_empty()
        || descriptor.title.len() > 128
        || descriptor.description.len() > 1024
        || descriptor.entry.len() > 256
        || !descriptor.entry.ends_with(".js")
        || descriptor.entry.contains(['\\', ':', '\0', '?', '#'])
        || descriptor
            .entry
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
        || descriptor.permissions.frontmatter_fields.len() > 32
        || descriptor
            .permissions
            .frontmatter_fields
            .iter()
            .any(|field| field.is_empty() || field.len() > 128)
    {
        return Err(invalid("Invalid background job declaration"));
    }
    descriptor
        .schedule
        .validate()
        .map_err(|message| invalid(&message))
}

pub(super) fn source(vault: &VaultService, job: &storage::StoredJob) -> Result<String> {
    let installed = discover(vault)?
        .into_iter()
        .find(|entry| entry.plugin == job.plugin && entry.descriptor.id == job.descriptor.id)
        .ok_or_else(|| invalid("Background plugin is disabled or unavailable"))?;
    if installed.fingerprint != job.configuration.fingerprint
        || installed.descriptor != job.descriptor
    {
        return Err(invalid(
            "Plugin changed; review its background permissions again",
        ));
    }
    let state = vault.lock()?;
    let records = read_json(&state.root, ".lapis/installed-plugins.json")?;
    let version = records["plugins"][&job.plugin]["installedVersion"]
        .as_str()
        .ok_or_else(|| invalid("Invalid plugin version"))?;
    let asset = assets::read(
        &state.root,
        &job.plugin,
        version,
        &installed.fingerprint,
        &job.descriptor.entry,
    )?;
    if asset.bytes.len() > 1024 * 1024 {
        return Err(invalid("Background handler exceeds bounds"));
    }
    String::from_utf8(asset.bytes).map_err(|_| invalid("Background handler is not UTF-8"))
}
