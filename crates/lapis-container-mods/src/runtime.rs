use crate::{archive, cache::Cache, config::Config, install, log, registry::Registry};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Serialize, Deserialize, Default)]
pub struct Receipts {
    pub mods: Vec<Receipt>,
}
#[derive(Serialize, Deserialize)]
pub struct Receipt {
    pub reference: String,
    pub manifest: String,
    pub layer: String,
    pub platform: String,
    pub paths: Vec<PathBuf>,
}
pub async fn load(config: &Config, registry: &Registry) -> Result<Receipts> {
    if config.references.is_empty() {
        return Ok(Receipts::default());
    }
    // SAFETY: geteuid has no preconditions; no privilege escalation is attempted.
    ensure!(
        unsafe { libc::geteuid() } == 0 || config.root != Path::new("/"),
        "Mods require root initialization; refusing privilege escalation"
    );
    let root = Dir::open_ambient_dir(&config.root, ambient_authority())?;
    // Check writability before network access. This is container-local, never app data.
    let state = config.root.join("var/lib/container-mods");
    for parent in [
        config.root.join("var"),
        config.root.join("var/lib"),
        state.clone(),
    ] {
        if let Ok(meta) = fs::symlink_metadata(&parent) {
            ensure!(
                !meta.file_type().is_symlink()
                    && (meta.uid() == 0 || meta.uid() == unsafe { libc::geteuid() })
                    && meta.mode() & 0o022 == 0,
                "unsafe receipt directory"
            );
        }
    }
    fs::create_dir_all(&state).context("Mods need a writable root filesystem")?;
    fs::set_permissions(&state, fs::Permissions::from_mode(0o700))?;
    let meta = fs::symlink_metadata(&state)?;
    ensure!(
        meta.uid() == unsafe { libc::geteuid() } && meta.mode() & 0o022 == 0,
        "unsafe receipt ownership"
    );
    let probe =
        tempfile::NamedTempFile::new_in(&state).context("Mods need a writable root filesystem")?;
    drop(probe);
    let cache = Cache::open(&config.cache)?;
    if config.debug {
        log(
            "debug",
            &format!(
                "strict={} offline={} requested={}",
                config.strict,
                config.offline,
                config.references.len()
            ),
        );
    }
    ensure!(
        !config.cache.starts_with(config.root.join("data")),
        "Mod cache must be independent of application data"
    );
    let receipt_path = state.join("applied.json");
    let old: Receipts = if receipt_path.exists() {
        let meta = fs::symlink_metadata(&receipt_path)?;
        ensure!(
            meta.is_file()
                && !meta.file_type().is_symlink()
                && meta.uid() == unsafe { libc::geteuid() }
                && meta.mode() & 0o022 == 0,
            "unsafe receipt file"
        );
        serde_json::from_slice(&fs::read(&receipt_path)?)?
    } else {
        Receipts::default()
    };
    struct Resolved {
        reference: String,
        artifact: crate::registry::Artifact,
        plan: archive::Plan,
    }
    let mut resolved: Vec<Resolved> = Vec::new();
    let mut claimed = std::collections::HashMap::<PathBuf, bool>::new();
    for reference in &config.references {
        log("resolve", &reference.to_string());
        let result = async {
            let artifact = registry.resolve(reference, &cache, config.offline).await?;
            if config.debug {
                log(
                    "resolved",
                    &format!(
                        "{} manifest={} layer={}",
                        artifact.platform, artifact.manifest, artifact.layer
                    ),
                );
            }
            let plan = archive::prepare(&artifact.blob, &artifact.media, &cache.path)?;
            ensure!(
                !plan
                    .entries
                    .iter()
                    .any(|entry| config.root.join(&entry.path).starts_with(&config.cache)),
                "Mod cannot modify its cache"
            );
            crate::links::validate(
                &root,
                resolved
                    .iter()
                    .flat_map(|entry| entry.plan.entries.iter())
                    .chain(plan.entries.iter()),
            )?;
            let mut next = claimed.clone();
            for entry in &plan.entries {
                let directory = matches!(entry.kind, archive::Kind::Directory);
                if let Some(previous_directory) = next.insert(entry.path.clone(), directory) {
                    ensure!(
                        (directory && previous_directory)
                            || (!directory
                                && !previous_directory
                                && plan.replace.contains(&entry.path)),
                        "Mod collision requires explicit replacement"
                    );
                }
            }
            Ok::<_, anyhow::Error>((
                Resolved {
                    reference: reference.to_string(),
                    artifact,
                    plan,
                },
                next,
            ))
        }
        .await;
        match result {
            Ok((entry, next)) => {
                resolved.push(entry);
                claimed = next;
            }
            Err(error) => {
                log("failed", &format!("{reference}: {error}"));
                if config.strict {
                    return Err(anyhow::anyhow!("requested Mod failed: {reference}"));
                }
            }
        }
    }
    // Only final effective files matter. This makes explicit later replacements
    // deterministic and restartable without rewriting obsolete intermediate bytes.
    let mut final_paths = std::collections::HashSet::new();
    for entry in resolved.iter_mut().rev() {
        for file in &entry.plan.entries {
            if !matches!(file.kind, archive::Kind::Directory)
                && !final_paths.insert(file.path.clone())
            {
                entry.plan.ignored.insert(file.path.clone());
            }
        }
        entry
            .plan
            .hooks
            .retain(|path| !entry.plan.ignored.contains(path));
    }
    let mut receipts = Receipts::default();
    let mut hooks: Vec<PathBuf> = Vec::new();
    for entry in resolved {
        let result = (|| -> Result<Receipt> {
            let prior = old.mods.iter().any(|old| {
                old.reference == entry.reference && old.manifest == entry.artifact.manifest
            });
            if !(prior && install::matches(&root, &entry.plan)) {
                install::apply(&root, &entry.plan)?;
                log("applied", &entry.artifact.manifest);
            } else {
                log("already-applied", &entry.artifact.manifest);
            }
            hooks.extend(entry.plan.hooks.iter().map(|path| config.root.join(path)));
            Ok(Receipt {
                reference: entry.reference.clone(),
                manifest: entry.artifact.manifest,
                layer: entry.artifact.layer,
                platform: entry.artifact.platform,
                paths: entry
                    .plan
                    .entries
                    .iter()
                    .filter(|file| !entry.plan.ignored.contains(&file.path))
                    .map(|file| file.path.clone())
                    .collect(),
            })
        })();
        match result {
            Ok(receipt) => {
                receipts.mods.push(receipt);
                cache.write(&receipt_path, &serde_json::to_vec_pretty(&receipts)?)?;
            }
            Err(error) => {
                log("failed", &format!("{}: {error}", entry.reference));
                if config.strict {
                    return Err(anyhow::anyhow!("requested Mod failed: {}", entry.reference));
                }
            }
        }
    }
    // All file layers are applied before any hook. Hook changes are trusted and not rollbackable.
    hooks.sort_by_key(|path| path.file_name().is_some_and(|name| name == "post-init.sh"));
    for path in hooks {
        log("hook", &path.display().to_string());
        let succeeded = Command::new(&path)
            .status()
            .is_ok_and(|status| status.success());
        if !succeeded {
            log("hook-failed", &path.display().to_string());
            ensure!(!config.strict, "Mod hook failed");
        }
    }
    Ok(receipts)
}
