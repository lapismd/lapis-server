use super::staged::{StagedFile, rename_exclusive};
use super::{VaultRoot, invalid, sync_directory};
use cap_std::fs::Dir;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;

impl VaultRoot {
    pub(super) fn entry_parent(&self, path: &str) -> io::Result<(Dir, OsString)> {
        let relative = self.checked(path, false)?;
        let name = relative
            .file_name()
            .ok_or_else(|| invalid("Missing filename"))?
            .to_owned();
        let parent = relative
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        Ok((self.dir.open_dir(parent)?, name))
    }

    pub fn rename(&self, source: &str, destination: &str) -> io::Result<()> {
        self.stat(source)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Rename source is missing"))?;
        let (from, old_name) = self.entry_parent(source)?;
        let (to, new_name) = self.entry_parent(destination)?;
        if source == destination {
            return Ok(());
        }
        rename_exclusive(&from, &old_name, &to, &new_name)?;
        sync_directory(&from)?;
        sync_directory(&to)
    }

    pub fn copy_file(&self, source: &str, destination: &str) -> io::Result<()> {
        let bytes = self.read(source)?;
        let permissions = self
            .dir
            .metadata(self.checked(source, false)?)?
            .permissions();
        let (parent, name) = self.entry_parent(destination)?;
        match parent.symlink_metadata(&name) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "Copy destination already exists",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let mut staged = StagedFile::new(&parent)?;
        staged.file.set_permissions(permissions)?;
        staged.file.write_all(&bytes)?;
        staged.file.sync_all()?;
        staged.publish(&name)?;
        sync_directory(&parent)
    }

    pub fn remove_file(&self, path: &str) -> io::Result<()> {
        if self.stat(path)?.is_some_and(|entry| entry.r#type != "file") {
            return Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                "File removal requires a regular file",
            ));
        }
        let (parent, name) = self.entry_parent(path)?;
        parent.remove_file(name)?;
        sync_directory(&parent)
    }

    pub fn remove_directory(&self, path: &str, recursive: bool) -> io::Result<()> {
        if self
            .stat(path)?
            .is_some_and(|entry| entry.r#type != "folder")
        {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "Directory removal requires a folder",
            ));
        }
        let (parent, name) = self.entry_parent(path)?;
        if recursive {
            parent.remove_dir_all(name)?;
        } else {
            parent.remove_dir(name)?;
        }
        sync_directory(&parent)
    }
}
