use super::conditional::WriteCondition;
#[cfg(target_os = "android")]
use super::staged::StagedFile;
use super::{MAX_FILE_BYTES, VaultRoot, invalid, sync_directory};
#[cfg(not(target_os = "android"))]
use cap_tempfile::TempFile;
use serde::Deserialize;
use std::io::{self, Read};

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriteOptions {
    pub mtime: Option<f64>,
}

impl VaultRoot {
    pub fn write(&self, path: &str, bytes: &[u8], options: &WriteOptions) -> io::Result<()> {
        self.write_from(path, bytes, options)
    }

    pub fn write_from(
        &self,
        path: &str,
        source: impl Read,
        options: &WriteOptions,
    ) -> io::Result<()> {
        self.stage_write(path, source, options, WriteCondition::Any)
    }

    pub fn write_if_unchanged(
        &self,
        path: &str,
        source: impl Read,
        expected: Option<&[u8]>,
        options: &WriteOptions,
    ) -> io::Result<()> {
        self.stage_write(path, source, options, WriteCondition::Contents(expected))
    }

    fn stage_write(
        &self,
        path: &str,
        source: impl Read,
        options: &WriteOptions,
        condition: WriteCondition<'_>,
    ) -> io::Result<()> {
        let relative = self.checked(path, false)?;
        let mtime = modification_time(options)?;
        let parent_path = relative
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        let parent = self.dir.open_dir(parent_path)?;
        let name = relative
            .file_name()
            .ok_or_else(|| invalid("Missing filename"))?;
        condition.verify(&parent, name, path)?;
        let original = match parent.metadata(name) {
            Ok(metadata) => {
                if !metadata.is_file() {
                    return Err(invalid("Destination is not a regular file"));
                }
                if metadata.permissions().readonly() {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "File is read-only",
                    ));
                }
                Some(metadata)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        // Android app sandboxes can deny publication of an unnamed tempfile.
        // Stage a named sibling there so the final replacement uses renameat.
        #[cfg(target_os = "android")]
        let mut staged = StagedFile::new(&parent)?;
        #[cfg(target_os = "android")]
        let file = &mut staged.file;
        #[cfg(not(target_os = "android"))]
        let mut staged = TempFile::new(&parent)?;
        #[cfg(not(target_os = "android"))]
        let file = staged.as_file_mut();
        if let Some(original) = original {
            file.set_permissions(original.permissions())?;
        }
        let count = io::copy(&mut source.take(MAX_FILE_BYTES + 1), file)?;
        if count > MAX_FILE_BYTES {
            return Err(invalid("Write exceeds the 64 MiB limit"));
        }
        if let Some(mtime) = mtime {
            file.try_clone()?.into_std().set_modified(mtime)?;
        }
        file.sync_all()?;
        condition.verify(&parent, name, path)?;
        staged.replace(name)?;
        sync_directory(&parent)?;
        Ok(())
    }

    pub fn append(&self, path: &str, data: &str, options: &WriteOptions) -> io::Result<()> {
        let mut bytes = match self.read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error),
        };
        bytes.extend_from_slice(data.as_bytes());
        self.write(path, &bytes, options)
    }
}

pub(super) fn modification_time(
    options: &WriteOptions,
) -> io::Result<Option<std::time::SystemTime>> {
    Ok(match options.mtime {
        Some(value) if value.is_finite() && (0.0..=253_402_300_799_999.0).contains(&value) => {
            Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs_f64(value / 1000.0))
        }
        Some(_) => return Err(invalid("Invalid modification time")),
        None => None,
    })
}
