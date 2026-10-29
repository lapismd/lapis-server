use super::{ArchiveLimits, invalid, zip_error};
use crate::vault::VaultRoot;
use std::{
    collections::BTreeMap,
    io::{self, Read, Seek, SeekFrom},
};

/// Only an empty, unpublished content root may be initialized by an archive.
/// On payload/CRC failure the host discards this provisional root, never admits it.
pub fn extract_zip(
    mut source: impl Read + Seek,
    root: &VaultRoot,
    limits: ArchiveLimits,
) -> io::Result<()> {
    root.verify_physical_root()?;
    if root.dir.entries()?.next().transpose()?.is_some() {
        return Err(invalid("Archive destination must be empty"));
    }
    if source.seek(SeekFrom::End(0))? > limits.archive_bytes {
        return Err(invalid("ZIP exceeds the compressed byte limit"));
    }
    source.rewind()?;
    let mut archive = zip::ZipArchive::new(source).map_err(zip_error)?;
    if archive.len() > limits.entries {
        return Err(invalid("ZIP exceeds the entry limit"));
    }
    let mut entries = Vec::new();
    let mut paths = BTreeMap::new();
    let mut expanded = 0u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(zip_error)?;
        let raw = entry.name().map_err(zip_error)?.replace('\\', "/");
        let directory = raw.ends_with('/');
        let path = if directory {
            &raw[..raw.len() - 1]
        } else {
            &raw
        };
        root.checked(path, false)?;
        let kind = entry.unix_mode().unwrap_or(0) & 0o170000;
        if entry.encrypted()
            || entry.is_symlink()
            || !matches!(kind, 0 | 0o040000 | 0o100000)
            || (kind == 0o040000 && !directory)
            || (kind == 0o100000 && directory)
        {
            return Err(invalid("ZIP contains encrypted, linked or special entries"));
        }
        if paths.insert(path.to_string(), directory).is_some() {
            return Err(invalid("ZIP contains conflicting paths"));
        }
        expanded = expanded
            .checked_add(entry.size())
            .ok_or_else(|| invalid("ZIP size overflow"))?;
        if expanded > limits.expanded_bytes {
            return Err(invalid("ZIP exceeds the expanded byte limit"));
        }
        if directory && entry.size() != 0 {
            return Err(invalid("ZIP directory contains file data"));
        }
        if !directory {
            entries.push((index, path.to_string(), entry.size()));
        }
    }
    // Check descendants without allocating every implicit parent of deep paths.
    for (path, directory) in &paths {
        if !directory {
            let prefix = format!("{path}/");
            if paths
                .range(prefix.clone()..)
                .next()
                .is_some_and(|(next, _)| next.starts_with(&prefix))
            {
                return Err(invalid("ZIP file collides with a directory"));
            }
        }
    }
    let mut actual = 0u64;
    for (index, path, size) in entries {
        let entry = archive.by_index(index).map_err(zip_error)?;
        if let Some(parent) = std::path::Path::new(&path)
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            root.dir.create_dir_all(parent)?;
        }
        let mut options = cap_std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        let mut output = root.dir.open_with(root.checked(&path, false)?, &options)?;
        let copied = io::copy(&mut entry.take(size.saturating_add(1)), &mut output)?;
        actual = actual
            .checked_add(copied)
            .ok_or_else(|| invalid("ZIP size overflow"))?;
        if copied != size || actual > limits.expanded_bytes {
            return Err(invalid("ZIP payload does not match its declared size"));
        }
        output.sync_all()?;
    }
    root.verify_physical_root()
}
