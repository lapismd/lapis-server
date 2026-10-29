use super::{
    MAX_FILE_BYTES, VaultRoot, WriteOptions, invalid, staged::StagedFile, sync_directory,
    writes::modification_time,
};
use std::io::{self, Read};

impl VaultRoot {
    pub fn create_file(
        &self,
        path: &str,
        source: impl Read,
        options: &WriteOptions,
    ) -> io::Result<()> {
        let mtime = modification_time(options)?;
        let (parent, name) = self.entry_parent(path)?;
        match parent.symlink_metadata(&name) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "Creation destination already exists",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let mut staged = StagedFile::new(&parent)?;
        let count = io::copy(&mut source.take(MAX_FILE_BYTES + 1), &mut staged.file)?;
        if count > MAX_FILE_BYTES {
            return Err(invalid("Create exceeds the 64 MiB limit"));
        }
        if let Some(mtime) = mtime {
            staged.file.try_clone()?.into_std().set_modified(mtime)?;
        }
        staged.file.sync_all()?;
        staged.publish(&name)?;
        sync_directory(&parent)
    }
}
