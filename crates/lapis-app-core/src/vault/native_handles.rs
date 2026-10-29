use super::VaultRoot;
use cap_std::fs::{Dir, Metadata};
use std::io;

impl VaultRoot {
    /// Native admission witnesses retain this capability; never send it to clients.
    pub fn directory_handle(&self) -> io::Result<Dir> {
        self.dir.try_clone()
    }

    /// Open only a checked vault-relative directory for native resource consumers.
    pub fn open_directory(&self, path: &str) -> io::Result<Dir> {
        self.dir.open_dir(self.checked(path, true)?)
    }

    pub fn validate_path(&self, path: &str, root_allowed: bool) -> io::Result<()> {
        self.checked(path, root_allowed).map(|_| ())
    }

    pub fn entry_metadata(&self, path: &str) -> io::Result<Metadata> {
        self.dir.symlink_metadata(self.checked(path, true)?)
    }
}
