use cap_std::fs::{Dir, File, OpenOptions};
use std::{ffi::OsStr, io};

pub(super) fn rename_exclusive(
    from: &Dir,
    source: &OsStr,
    to: &Dir,
    destination: &OsStr,
) -> io::Result<()> {
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "android"
    ))]
    {
        rustix::fs::renameat_with(
            from,
            source,
            to,
            destination,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(Into::into)
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "android"
    )))]
    {
        let _ = (from, source, to, destination);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Exclusive rename is unavailable on this platform",
        ))
    }
}

pub struct StagedFile<'a> {
    parent: &'a Dir,
    name: String,
    pub file: File,
    published: bool,
}
impl Drop for StagedFile<'_> {
    fn drop(&mut self) {
        if !self.published {
            let _ = self.parent.remove_file(&self.name);
        }
    }
}

impl<'a> StagedFile<'a> {
    pub fn new(parent: &'a Dir) -> io::Result<Self> {
        let name = format!(".lapis-stage-{}", uuid::Uuid::new_v4());
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let file = parent.open_with(&name, &options)?;
        Ok(Self {
            parent,
            name,
            file,
            published: false,
        })
    }
    pub fn publish(mut self, destination: &OsStr) -> io::Result<()> {
        rename_exclusive(
            self.parent,
            OsStr::new(&self.name),
            self.parent,
            destination,
        )?;
        self.published = true;
        Ok(())
    }
    pub fn replace(mut self, destination: &OsStr) -> io::Result<()> {
        self.parent.rename(&self.name, self.parent, destination)?;
        self.published = true;
        Ok(())
    }
}
