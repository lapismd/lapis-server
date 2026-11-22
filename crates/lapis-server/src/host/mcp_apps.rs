use super::Host;
use lapis_app_core::{services::plugin_operations::OperationProvider, vault::VaultRoot};
use lapis_content_mcp::{AppResource, MAX_HTML_BYTES};
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
        self.mcp_apps.register_provider(
            &lapis_task_operations::TaskOperations.descriptors(),
            "tasks",
            "list",
            &["get", "plan", "deadline", "inbox", "complete", "inspect"],
            resource(
                &root,
                "tasks.html",
                "ui://lapis/tasks/task-list-v1",
                "Lapis Tasks",
            )?,
        )?;
        Ok(self)
    }
}

fn resource(root: &VaultRoot, name: &str, uri: &str, title: &str) -> io::Result<AppResource> {
    root.verify_physical_root()?;
    root.validate_path(name, false)?;
    if !root.entry_metadata(name)?.is_file() {
        return Err(io::Error::other("MCP App bundle must be a regular file"));
    }
    let file = root.directory_handle()?.open(name)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("MCP App bundle must be a regular file"));
    }
    let mut html = String::new();
    file.take(MAX_HTML_BYTES as u64 + 1)
        .read_to_string(&mut html)?;
    root.verify_physical_root()?;
    Ok(AppResource {
        uri: uri.into(),
        title: title.into(),
        html,
    })
}
