use crate::{
    services::{Error, Result},
    vault::VaultRoot,
};
use lapis_app_protocol::ErrorCode;

pub(in crate::services) fn collect(
    root: &VaultRoot,
    cancelled: impl Fn() -> bool,
) -> Result<Option<Vec<String>>> {
    let mut directories = vec![String::new()];
    let mut files = Vec::new();
    let mut bytes = 0usize;
    let mut entries = 0usize;
    while let Some(directory) = directories.pop() {
        if cancelled() {
            return Ok(None);
        }
        root.verify_physical_root()?;
        let listing = root.list(&directory)?;
        entries = entries
            .saturating_add(listing.folders.len())
            .saturating_add(listing.files.len());
        if entries > 100_000 {
            return Err(bounds());
        }
        let child = |name: String| {
            if directory.is_empty() {
                name
            } else {
                format!("{directory}/{name}")
            }
        };
        directories.extend(
            listing
                .folders
                .into_iter()
                .filter(|name| !name.starts_with('.'))
                .map(child),
        );
        for name in listing
            .files
            .into_iter()
            .filter(|name| !name.starts_with('.'))
        {
            let path = child(name);
            bytes = bytes.saturating_add(path.len());
            if files.len() >= 100_000 || bytes > 32 * 1024 * 1024 {
                return Err(bounds());
            }
            files.push(path);
        }
    }
    files.sort();
    Ok(Some(files))
}
fn bounds() -> Error {
    Error::new(
        ErrorCode::InvalidRequest,
        "Native metadata manifest exceeds bounds",
    )
}
