//! Bounded capability-relative private records, never repository or browser state.
use super::*;
use crate::vault::{VaultRoot, WriteOptions};
use serde::{Serialize, de::DeserializeOwned};
const MAX: usize = 128 * 1024;
pub(super) fn load<T: DeserializeOwned>(
    selection: &Selection,
    path: &'static str,
) -> Result<Option<T>> {
    let root = VaultRoot::open(selection.private_path())?;
    root.verify_physical_root()?;
    let Some(metadata) = root.stat(path)? else {
        return Ok(None);
    };
    if metadata.r#type != "file" || metadata.size > MAX as u64 {
        return Err(invalid());
    }
    let bytes = root.read_range(path, 0, MAX + 1)?;
    if bytes.len() > MAX {
        return Err(invalid());
    }
    let value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    root.verify_physical_root()?;
    Ok(Some(value))
}
pub(super) fn save<T: Serialize>(
    selection: &Selection,
    path: &'static str,
    value: &T,
) -> Result<()> {
    let root = VaultRoot::open(selection.private_path())?;
    root.verify_physical_root()?;
    let before = match root.stat(path)? {
        None => None,
        Some(metadata) if metadata.r#type == "file" && metadata.size <= MAX as u64 => {
            Some(root.read_range(path, 0, MAX + 1)?)
        }
        _ => return Err(invalid()),
    };
    let bytes = serde_json::to_vec(value).map_err(|_| invalid())?;
    if bytes.len() > MAX {
        return Err(invalid());
    }
    root.write_if_unchanged(
        path,
        &bytes[..],
        before.as_deref(),
        &WriteOptions::default(),
    )?;
    root.verify_physical_root()?;
    Ok(())
}
pub(super) fn invalid() -> Error {
    Error::new(ErrorCode::Io, "Saved repository review is unavailable")
}
