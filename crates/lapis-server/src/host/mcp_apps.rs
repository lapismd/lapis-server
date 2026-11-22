use super::Host;
use lapis_app_core::vault::VaultRoot;
use lapis_content_mcp::{AppResource, MAX_HTML_BYTES};
use serde::Deserialize;
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
};

impl Host {
    /// Read installation-owned bundles once, before mounting the MCP adapter.
    pub fn with_mcp_apps(mut self, root: &Path, excluded: &[PathBuf]) -> io::Result<Self> {
        let root = VaultRoot::open(root)?;
        for path in excluded {
            let path = path.canonicalize()?;
            if root.path().starts_with(&path) || path.starts_with(root.path()) {
                return Err(io::Error::other(
                    "MCP Apps must use a separate installation directory",
                ));
            }
        }
        self.mcp_apps.register_builtin(
            "read_note",
            resource(
                &root,
                "notes.html",
                "ui://lapis/notes/read-note-v1",
                "Lapis Notes",
            )?,
        )?;
        let manifests = match read_file(&root, "providers.json", 64 * 1024) {
            Ok(json) => serde_json::from_str::<Vec<ProviderManifest>>(&json)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error),
        };
        let descriptors = self
            .content
            .operations
            .installed_descriptors()
            .cloned()
            .collect::<Vec<_>>();
        for manifest in manifests {
            if manifest.version != 1 || !identifier(&manifest.provider) || manifest.apps.len() > 32
            {
                return Err(io::Error::other("Invalid provider MCP App manifest"));
            }
            for app in manifest.apps {
                if !app.file.strip_suffix(".html").is_some_and(identifier) {
                    return Err(io::Error::other("Invalid provider MCP App filename"));
                }
                self.mcp_apps.register_provider(
                    &descriptors,
                    &manifest.provider,
                    &app.operation,
                    &app.actions.iter().map(String::as_str).collect::<Vec<_>>(),
                    resource(
                        &root,
                        &format!("providers/{}/{}", manifest.provider, app.file),
                        &app.resource_uri,
                        &app.title,
                    )?,
                )?;
            }
        }
        Ok(self)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderManifest {
    version: u8,
    provider: String,
    apps: Vec<ProviderApp>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderApp {
    operation: String,
    actions: Vec<String>,
    resource_uri: String,
    title: String,
    file: String,
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn resource(root: &VaultRoot, name: &str, uri: &str, title: &str) -> io::Result<AppResource> {
    Ok(AppResource {
        uri: uri.into(),
        title: title.into(),
        html: read_file(root, name, MAX_HTML_BYTES)?,
    })
}

fn read_file(root: &VaultRoot, name: &str, limit: usize) -> io::Result<String> {
    root.verify_physical_root()?;
    root.validate_path(name, false)?;
    if !root.entry_metadata(name)?.is_file() {
        return Err(io::Error::other("MCP App input must be a regular file"));
    }
    let file = root.directory_handle()?.open(name)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("MCP App input must be a regular file"));
    }
    let mut contents = String::new();
    file.take((limit + 1) as u64)
        .read_to_string(&mut contents)?;
    root.verify_physical_root()?;
    if contents.len() > limit {
        return Err(io::Error::other("MCP App input exceeds the size limit"));
    }
    Ok(contents)
}
