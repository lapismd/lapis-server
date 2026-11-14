use super::*;
use crate::vault::WriteOptions;
const RECEIPT: &str = ".agents/.lapis-skills.json";
fn read_receipt(root: &VaultRoot) -> io::Result<(Receipt, Option<String>)> {
    if root.stat(RECEIPT)?.is_none() {
        return Ok((
            Receipt {
                schema_version: 1,
                skills: BTreeMap::new(),
            },
            None,
        ));
    }
    let text = root.read_text(RECEIPT)?;
    let receipt: Receipt =
        serde_json::from_str(&text).map_err(|_| invalid("Invalid skill installation receipt"))?;
    if receipt.schema_version != 1
        || receipt.skills.iter().any(|(id, record)| {
            !name(id)
                || record.source.is_empty()
                || record.version.is_empty()
                || record.files.is_empty()
                || record.files.iter().any(|(file, hash)| {
                    !path(file)
                        || hash.len() != 64
                        || !hash
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
        })
    {
        return Err(invalid("Invalid skill installation receipt"));
    }
    Ok((receipt, Some(text)))
}
fn save(root: &VaultRoot, receipt: &Receipt, previous: &mut Option<String>) -> io::Result<()> {
    root.mkdir(".agents", true)?;
    let text =
        serde_json::to_string_pretty(receipt).map_err(|_| invalid("Invalid skill receipt"))? + "\n";
    root.write_if_unchanged(
        RECEIPT,
        text.as_bytes(),
        previous.as_deref().map(str::as_bytes),
        &WriteOptions::default(),
    )?;
    *previous = Some(text);
    Ok(())
}
/// Caller holds the vault writer lock and admits the source bundles.
/// A durable pending record fences interrupted operations; this never retries them.
pub fn manage(
    root: &VaultRoot,
    bundles: &[PackagedSkill],
    action: &str,
    selected: Option<&str>,
) -> io::Result<Vec<SkillResult>> {
    if !["list", "install", "update"].contains(&action) {
        return Err(invalid("Invalid skill action"));
    }
    root.verify_physical_root()?;
    let mut seen = std::collections::BTreeSet::new();
    for bundle in bundles {
        if !name(&bundle.name)
            || bundle.source.is_empty()
            || bundle.version.is_empty()
            || !bundle.files.contains_key("SKILL.md")
            || bundle.files.len() > 128
            || bundle
                .files
                .iter()
                .any(|(file, text)| !path(file) || text.len() > 1024 * 1024)
        {
            return Err(invalid("Invalid packaged skill"));
        }
        if !seen.insert(&bundle.name) {
            return Err(invalid("Duplicate skill contribution"));
        }
    }
    if selected.is_some_and(|id| !seen.iter().any(|name| name.as_str() == id)) {
        return Err(invalid("Skill is unavailable"));
    }
    let (mut receipt, mut text) = read_receipt(root)?;
    let mut results = Vec::new();
    for bundle in bundles
        .iter()
        .filter(|b| selected.is_none_or(|id| b.name == id))
    {
        let directory = format!(".agents/skills/{}", bundle.name);
        let exists = root.stat(&directory)?.is_some();
        let current = snapshot(root, &directory)?;
        let actual = hashes(&current);
        let incoming = hashes(&bundle.files);
        let installed = receipt.skills.get(&bundle.name);
        let reason = if installed.is_some_and(|r| r.pending) {
            Some("interrupted")
        } else if exists && installed.is_none() {
            Some("unmanaged")
        } else if installed.is_some_and(|r| r.source != bundle.source) {
            Some("owner-changed")
        } else if installed.is_some_and(|r| !exists || r.files != actual) {
            Some("locally-modified")
        } else {
            None
        };
        let mut result = SkillResult {
            name: bundle.name.clone(),
            source: bundle.source.clone(),
            version: bundle.version.clone(),
            status: "available".into(),
            reason: reason.map(str::to_owned),
        };
        if reason.is_some() {
            result.status = "skipped".into();
        } else if exists && actual == incoming {
            result.status = "unchanged".into();
        } else if action == "list" {
        } else if action == "install" && exists {
            result.status = "skipped".into();
            result.reason = Some("update-available".into());
        } else if action == "update" && installed.is_none() {
            result.status = "skipped".into();
            result.reason = Some("not-installed".into());
        } else {
            receipt.skills.insert(
                bundle.name.clone(),
                Record {
                    source: bundle.source.clone(),
                    version: bundle.version.clone(),
                    files: incoming.clone(),
                    pending: true,
                },
            );
            save(root, &receipt, &mut text)?;
            if hashes(&snapshot(root, &directory)?) != actual {
                return Err(invalid("Skill changed during installation"));
            }
            for (file, body) in &bundle.files {
                let destination = format!("{directory}/{file}");
                root.mkdir(destination.rsplit_once('/').unwrap().0, true)?;
                root.write_if_unchanged(
                    &destination,
                    body.as_bytes(),
                    current.get(file).map(String::as_bytes),
                    &WriteOptions::default(),
                )?;
            }
            for (file, body) in current
                .iter()
                .filter(|(file, _)| !bundle.files.contains_key(*file))
            {
                let destination = format!("{directory}/{file}");
                if root.read_text(&destination)? != *body {
                    return Err(invalid("Skill changed during installation"));
                }
                root.remove_file(&destination)?;
            }
            if hashes(&snapshot(root, &directory)?) != incoming {
                return Err(invalid("Skill changed during installation"));
            }
            receipt.skills.get_mut(&bundle.name).unwrap().pending = false;
            save(root, &receipt, &mut text)?;
            result.status = if exists { "updated" } else { "installed" }.into();
        }
        results.push(result);
    }
    Ok(results)
}
