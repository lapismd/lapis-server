mod assets;
mod health;
mod paths;
pub(super) use assets::serve;
pub(super) use health::{available, health, ready};

use lapis_app_core::vault::VaultRoot;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

/// Installation-selected public build output, never admitted vault authority.
pub struct StaticWeb {
    root: VaultRoot,
}
pub(super) struct Asset {
    file: File,
    length: u64,
    etag: String,
}
impl StaticWeb {
    pub fn open(root: &Path, private_paths: &[PathBuf]) -> io::Result<Self> {
        let root = VaultRoot::open(root)?;
        for private in private_paths {
            let private = private.canonicalize()?;
            if root.path().starts_with(&private) || private.starts_with(root.path()) {
                return Err(io::Error::other(
                    "Static build must stay separate from private installation data",
                ));
            }
        }
        let web = Self { root };
        if web.asset("index.html")?.length == 0 {
            return Err(io::Error::other("Static application index is unavailable"));
        }
        Ok(web)
    }

    fn asset(&self, path: &str) -> io::Result<Asset> {
        self.root.verify_physical_root()?;
        self.root.validate_path(path, false)?;
        if !self.root.entry_metadata(path)?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Static entry is not a regular file",
            ));
        }
        let mut file = self.root.directory_handle()?.open(path)?.into_std();
        let before = file.metadata()?;
        if !before.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Static entry is not a regular file",
            ));
        }
        let mut hash = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        loop {
            let length = file.read(&mut buffer)?;
            if length == 0 {
                break;
            }
            hash.update(&buffer[..length]);
        }
        let after = file.metadata()?;
        if before.len() != after.len() || before.modified()? != after.modified()? {
            return Err(io::Error::other("Static build changed during read"));
        }
        self.root.verify_physical_root()?;
        file.seek(SeekFrom::Start(0))?;
        Ok(Asset {
            file,
            length: before.len(),
            etag: format!("\"{:x}\"", hash.finalize()),
        })
    }
}
