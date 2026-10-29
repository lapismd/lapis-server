use super::{VaultRoot, invalid};
use cap_std::fs::{File, OpenOptions};
use std::{
    io,
    path::{Path, PathBuf},
};

/// Keep the admitted physical object alive until its OS handoff completes.
pub struct ActionTarget {
    file: File,
    path: PathBuf,
}

impl ActionTarget {
    pub fn is_directory(&self) -> io::Result<bool> {
        Ok(self.file.metadata()?.is_dir())
    }

    pub fn verified_path(&self) -> io::Result<PathBuf> {
        self.validate_path(&self.path)?;
        Ok(self.path.clone())
    }

    /// Native file-reference URLs may resolve a renamed selected object.
    pub fn validate_path(&self, path: &Path) -> io::Result<()> {
        #[cfg(unix)]
        {
            use cap_std::fs::MetadataExt;
            use std::os::unix::fs::MetadataExt as _;
            let selected = self.file.metadata()?;
            let current = std::fs::metadata(path)?;
            if selected.dev() == current.dev() && selected.ino() == current.ino() {
                return Ok(());
            }
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Selected file changed before native action",
            ))
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Native file identity is unavailable",
            ))
        }
    }
}

impl VaultRoot {
    pub fn action_target(&self, path: &str, root_allowed: bool) -> io::Result<ActionTarget> {
        let relative = self.checked(path, root_allowed)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = self.dir.open_with(&relative, &options)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() && !metadata.is_dir() {
            return Err(invalid("Native actions require a regular file or folder"));
        }
        Ok(ActionTarget {
            file,
            path: self.path().join(if path.is_empty() { "" } else { path }),
        })
    }
}
