use anyhow::{Result, bail, ensure};
use oci_client::Reference;
use std::{collections::HashSet, path::PathBuf};

#[derive(Clone)]
pub struct Config {
    pub references: Vec<Reference>,
    pub cache: PathBuf,
    pub root: PathBuf,
    pub strict: bool,
    pub debug: bool,
    pub offline: bool,
}
impl Config {
    pub fn from_env() -> Result<Self> {
        Self::parse(|key| std::env::var(key).ok())
    }
    pub fn parse(env: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let value = env("DOCKER_MODS").unwrap_or_default();
        let mut references = Vec::new();
        let mut seen = HashSet::new();
        if !value.trim().is_empty() {
            for raw in value.split('|') {
                let raw = raw.trim();
                ensure!(
                    !raw.is_empty() && raw.len() <= 512 && !raw.contains(char::is_whitespace),
                    "invalid Mod reference"
                );
                let reference: Reference = raw.parse()?;
                if let Some(digest) = reference.digest() {
                    crate::cache::digest_name(digest)?;
                }
                ensure!(
                    seen.insert(reference.to_string()),
                    "duplicate Mod reference"
                );
                references.push(reference);
            }
        }
        ensure!(references.len() <= 32, "at most 32 Mods are supported");
        let cache = PathBuf::from(
            env("DOCKER_MODS_CACHE").unwrap_or_else(|| "/var/cache/container-mods".into()),
        );
        ensure!(
            cache.is_absolute()
                && cache.components().all(|part| matches!(
                    part,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )),
            "DOCKER_MODS_CACHE must be absolute without traversal"
        );
        Ok(Self {
            references,
            cache,
            root: "/".into(),
            strict: boolean(&env, "DOCKER_MODS_STRICT", true)?,
            debug: boolean(&env, "DOCKER_MODS_DEBUG", false)?,
            offline: boolean(&env, "DOCKER_MODS_OFFLINE", false)?,
        })
    }
}
fn boolean(env: &impl Fn(&str) -> Option<String>, name: &str, default: bool) -> Result<bool> {
    match env(name).as_deref() {
        None => Ok(default),
        Some("1" | "true") => Ok(true),
        Some("0" | "false") => Ok(false),
        _ => bail!("{name} must be true/false or 1/0"),
    }
}
