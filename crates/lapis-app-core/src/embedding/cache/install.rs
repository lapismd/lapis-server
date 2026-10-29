use super::{Cache, catalog::Asset, remote::Fetch};
use crate::{embedding::unavailable, services::Result};
use cap_tempfile::TempFile;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    time::{Duration, Instant},
};

impl Cache {
    pub(super) fn lease(&self, timeout: Duration) -> Result<File> {
        self.root.verify_physical_root()?;
        let name = self.root.checked("install.lock", false)?;
        let mut options = cap_std::fs::OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        // Two first-time installers must create one persistent lease atomically.
        // Reopen an existing lease without O_CREAT or truncation: concurrent
        // O_CREAT opens can report ENOENT on the supported macOS filesystem.
        let lease = match self.root.dir.open_with(&name, &options) {
            Ok(lease) => lease,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                options.create_new(false);
                self.root.dir.open_with(name, &options)?
            }
            Err(error) => return Err(error.into()),
        }
        .into_std();
        if !lease.metadata()?.is_file() {
            return Err(unavailable("Native embedding cache lease is invalid"));
        }
        let deadline = Instant::now() + timeout;
        loop {
            match lease.try_lock() {
                Ok(()) => return Ok(lease),
                Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => return Err(unavailable("Native embedding cache is busy")),
            }
        }
    }

    fn verified(&self, asset: &Asset) -> Result<bool> {
        self.root.verify_physical_root()?;
        let name = self.root.checked(&asset.filename(), false)?;
        let file = match self.root.dir.open(name) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        if !file.metadata()?.is_file() {
            return Err(unavailable("Native embedding asset is not a regular file"));
        }
        if file.metadata()?.len() != asset.bytes {
            return Ok(false);
        }
        let mut digest = Sha256::new();
        let count = std::io::copy(&mut file.take(asset.bytes + 1), &mut digest)?;
        Ok(count == asset.bytes && format!("{:x}", digest.finalize()) == asset.sha256)
    }

    pub(super) fn ensure(
        &self,
        asset: &Asset,
        remote: bool,
        fetch: &mut impl Fetch,
        progress: &mut dyn FnMut(u64),
    ) -> Result<()> {
        if asset.bytes == 0
            || asset.bytes > 64 * 1024 * 1024
            || asset.sha256.len() != 64
            || !asset.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || asset.name.is_empty()
            || asset.name.starts_with('.')
            || !asset
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        {
            return Err(unavailable("Native embedding asset manifest is invalid"));
        }
        if self.verified(asset)? {
            return Ok(());
        }
        if !remote {
            return Err(unavailable(
                "Native embedding asset is missing or corrupt; downloads are disabled",
            ));
        }
        let mut response = fetch.open(asset)?.take(asset.bytes + 1);
        let mut staged = TempFile::new(&self.root.dir)?;
        let mut digest = Sha256::new();
        let mut count = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let size = response
                .read(&mut buffer)
                .map_err(|_| unavailable("Native embedding asset download was interrupted"))?;
            if size == 0 {
                break;
            }
            count += size as u64;
            if count > asset.bytes {
                return Err(unavailable("Native embedding asset length differs"));
            }
            staged.write_all(&buffer[..size])?;
            digest.update(&buffer[..size]);
            progress(count);
        }
        if count != asset.bytes || format!("{:x}", digest.finalize()) != asset.sha256 {
            return Err(unavailable("Native embedding asset verification failed"));
        }
        staged.as_file().sync_all()?;
        self.root.verify_physical_root()?;
        self.root.checked(&asset.filename(), false)?;
        staged.replace(asset.filename())?;
        crate::vault::sync_directory(&self.root.dir)?;
        Ok(())
    }
}
