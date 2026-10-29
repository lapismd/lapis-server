use super::{ArchiveLimits, ArchiveSnapshot, invalid, zip_error};
use crate::vault::VaultRoot;
use std::io::{self, Read, Seek, SeekFrom, Write};

struct Bounded {
    file: std::fs::File,
    limit: u64,
}
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .file
            .stream_position()?
            .saturating_add(bytes.len() as u64)
            > self.limit
        {
            return Err(invalid("ZIP exceeds the compressed byte limit"));
        }
        self.file.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
impl Seek for Bounded {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file.seek(position)
    }
}

/// A private, complete content snapshot; network consumers retain no vault lease.
pub fn create_zip(root: &VaultRoot, limits: ArchiveLimits) -> io::Result<ArchiveSnapshot> {
    root.verify_physical_root()?;
    let mut pending = vec![String::new()];
    let mut paths = Vec::new();
    let mut count = 0usize;
    while let Some(parent) = pending.pop() {
        let directory = root.dir.open_dir(root.checked(&parent, true)?)?;
        for entry in directory.entries()? {
            let entry = entry?;
            count += 1;
            if count > limits.entries {
                return Err(invalid("Vault exceeds the archive entry limit"));
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid("Filename is not UTF-8"))?;
            let path = if parent.is_empty() {
                name
            } else {
                format!("{parent}/{name}")
            };
            root.checked(&path, false)?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() {
                paths.push(path);
            } else {
                return Err(invalid("Vault export contains a link or special file"));
            }
        }
    }
    paths.sort();
    let mut writer = zip::ZipWriter::new(Bounded {
        file: tempfile::tempfile()?,
        limit: limits.archive_bytes,
    });
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut expanded = 0u64;
    for path in paths {
        let source = root.open_file(&path)?;
        if source.metadata()?.len() > limits.expanded_bytes.saturating_sub(expanded) {
            return Err(invalid("Vault exceeds the expanded byte limit"));
        }
        writer.start_file(path, options).map_err(zip_error)?;
        expanded += io::copy(
            &mut source.take(limits.expanded_bytes - expanded + 1),
            &mut writer,
        )?;
        if expanded > limits.expanded_bytes {
            return Err(invalid("Vault exceeds the expanded byte limit"));
        }
    }
    let mut file = writer.finish().map_err(zip_error)?.file;
    let length = file.seek(SeekFrom::End(0))?;
    if length > limits.archive_bytes {
        return Err(invalid("ZIP exceeds the compressed byte limit"));
    }
    file.rewind()?;
    root.verify_physical_root()?;
    Ok(ArchiveSnapshot { file, length })
}
