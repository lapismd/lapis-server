//! Packaged workflow installation. Hosts select sources; skill files grant no authority.
mod install;
pub mod provider;
pub mod sources;
use crate::vault::VaultRoot;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackagedSkill {
    pub name: String,
    pub source: String,
    pub version: String,
    pub files: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillResult {
    pub name: String,
    pub source: String,
    pub version: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    schema_version: u32,
    skills: BTreeMap<String, Record>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    source: String,
    version: String,
    files: BTreeMap<String, String>,
    #[serde(default)]
    pending: bool,
}
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn name(value: &str) -> bool {
    value.len() <= 64
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b))
}
fn path(value: &str) -> bool {
    !value.is_empty()
        && !value.contains(['\\', '\0', ':'])
        && value
            .split('/')
            .all(|part| !matches!(part, "" | "." | ".."))
}
fn digest(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}
fn hashes<T: AsRef<[u8]>>(files: &BTreeMap<String, T>) -> BTreeMap<String, String> {
    files
        .iter()
        .map(|(path, body)| (path.clone(), digest(body.as_ref())))
        .collect()
}
fn snapshot(root: &VaultRoot, directory: &str) -> io::Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    if root.stat(directory)?.is_none() {
        return Ok(files);
    }
    fn walk(
        root: &VaultRoot,
        directory: &str,
        prefix: &str,
        files: &mut BTreeMap<String, Vec<u8>>,
    ) -> io::Result<()> {
        let dir = root.open_directory(directory)?;
        for entry in dir.entries()? {
            let entry = entry?;
            let child = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid("Invalid skill filename"))?;
            if !path(&child) || child.contains('/') {
                return Err(invalid("Invalid skill path"));
            }
            let full = format!("{directory}/{child}");
            let relative = format!("{prefix}{child}");
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                return Err(invalid("Skill symlinks are unavailable"));
            }
            if kind.is_dir() {
                walk(root, &full, &format!("{relative}/"), files)?;
            } else if kind.is_file() {
                if files.len() >= 128 {
                    return Err(invalid("Skill exceeds file limit"));
                }
                let body = root.read(&full)?;
                if body.len() > 1024 * 1024 {
                    return Err(invalid("Skill file exceeds limit"));
                }
                files.insert(relative, body);
            } else {
                return Err(invalid("Unsupported skill file"));
            }
        }
        Ok(())
    }
    walk(root, directory, "", &mut files)?;
    Ok(files)
}
pub use install::manage;
