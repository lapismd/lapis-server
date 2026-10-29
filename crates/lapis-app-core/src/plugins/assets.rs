//! Verified installed content, shared by Tauri and hosted adapters.
use crate::vault::{MAX_FILE_BYTES, VaultRoot};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{self, Read},
};

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledState {
    schema_version: u32,
    plugins: HashMap<String, InstalledPlugin>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledPlugin {
    plugin_id: String,
    installed_version: String,
    files: Vec<InstalledFile>,
}

#[derive(Deserialize)]
struct InstalledFile {
    path: String,
    size: u64,
    sha256: String,
}

pub struct PluginAsset {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
    pub sha256: String,
}

fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

fn hash(value: &str) -> io::Result<String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid("Invalid plugin asset digest"));
    }
    Ok(value.to_ascii_lowercase())
}

fn safe_path(path: &str) -> io::Result<()> {
    if path.len() > 4096
        || path.contains(['\\', '\0', ':', '?', '#'])
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(invalid("Invalid plugin asset path"));
    }
    Ok(())
}

fn content_type(path: &str) -> Option<&'static str> {
    match path.rsplit('.').next()?.to_ascii_lowercase().as_str() {
        "mjs" | "js" | "cjs" => Some("text/javascript; charset=utf-8"),
        "css" => Some("text/css; charset=utf-8"),
        "json" => Some("application/json; charset=utf-8"),
        "wasm" => Some("application/wasm"),
        "svg" => Some("image/svg+xml"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}

fn fingerprint(files: &[InstalledFile]) -> io::Result<String> {
    if files.is_empty() || files.len() > 4096 {
        return Err(invalid("Invalid installed plugin file count"));
    }
    let mut paths = std::collections::HashSet::new();
    let mut supported = Vec::new();
    for file in files {
        safe_path(&file.path)?;
        if file.size > MAX_FILE_BYTES || !paths.insert(&file.path) {
            return Err(invalid("Invalid installed plugin file metadata"));
        }
        let sha256 = hash(&file.sha256)?;
        if content_type(&file.path).is_some() {
            supported.push((&file.path, file.size, sha256));
        }
    }
    // Match the public browser contract, including astral Unicode paths.
    supported.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    let payload = supported
        .into_iter()
        .map(|(path, size, sha256)| format!("{path}:{size}:{sha256}"))
        .collect::<Vec<_>>()
        .join("|");
    Ok(digest(payload))
}

/// Caller owns physical admission and the shared native writer guard.
pub fn read(
    root: &VaultRoot,
    plugin: &str,
    version: &str,
    requested_fingerprint: &str,
    path: &str,
) -> io::Result<PluginAsset> {
    if plugin.is_empty()
        || plugin.len() > 128
        || !plugin
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || version.is_empty()
        || version.len() > 128
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-+".contains(&byte))
    {
        return Err(invalid("Invalid plugin coordinates"));
    }
    safe_path(path)?;
    let mime = content_type(path).ok_or_else(|| invalid("Unsupported plugin asset type"))?;
    root.verify_physical_root()?;
    let mut metadata = Vec::new();
    root.open_file(".lapis/installed-plugins.json")?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut metadata)?;
    if metadata.len() > 4 * 1024 * 1024 {
        return Err(invalid("Installed plugin metadata is too large"));
    }
    let state: InstalledState = serde_json::from_slice(&metadata)
        .map_err(|_| invalid("Installed plugin metadata is invalid"))?;
    let record = state
        .plugins
        .get(plugin)
        .ok_or_else(|| invalid("Plugin is not installed"))?;
    if state.schema_version != 1
        || record.plugin_id != plugin
        || record.installed_version != version
        || fingerprint(&record.files)? != hash(requested_fingerprint)?
    {
        return Err(invalid("Plugin installation identity changed"));
    }
    let file = record
        .files
        .iter()
        .find(|file| file.path == path)
        .ok_or_else(|| invalid("Plugin asset is not declared"))?;
    let directory = root.open_directory(&format!(".lapis/plugins/{plugin}"))?;
    root.validate_path(&format!(".lapis/plugins/{plugin}/{path}"), false)?;
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true);
    use cap_std::fs::OpenOptionsExt;
    options.custom_flags(libc::O_NONBLOCK);
    let handle = directory.open_with(path, &options)?;
    let actual = handle.metadata()?;
    if !actual.is_file() || actual.len() > MAX_FILE_BYTES {
        return Err(invalid("Plugin asset is too large"));
    }
    let mut bytes = Vec::new();
    handle.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    let sha256 = digest(&bytes);
    if bytes.len() as u64 != file.size || sha256 != hash(&file.sha256)? {
        return Err(invalid("Plugin asset bytes changed"));
    }
    Ok(PluginAsset {
        bytes,
        content_type: mime,
        sha256,
    })
}
