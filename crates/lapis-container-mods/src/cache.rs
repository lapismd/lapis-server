use anyhow::{Result, ensure};
use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

pub const MAX_BLOB: u64 = 1024 * 1024 * 1024;
pub fn digest_name(digest: &str) -> Result<&str> {
    let name = digest.strip_prefix("sha256:").unwrap_or("");
    ensure!(
        name.len() == 64
            && name
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "only lowercase SHA-256 digests are supported"
    );
    Ok(name)
}
pub fn verified(path: &Path, expected: &str, max: u64) -> Result<()> {
    digest_name(expected)?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    ensure!(
        file.metadata()?.is_file() && file.metadata()?.len() <= max,
        "invalid cached artifact size/type"
    );
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    ensure!(
        format!("sha256:{:x}", hash.finalize()) == expected,
        "artifact digest mismatch"
    );
    Ok(())
}
pub struct Cache {
    pub path: PathBuf,
    _lock: File,
}
impl Cache {
    pub fn open(path: &Path) -> Result<Self> {
        // Refuse linked or writable-by-other ancestors before creating state.
        let mut current = PathBuf::new();
        for component in path.components() {
            current.push(component);
            if let Ok(meta) = fs::symlink_metadata(&current) {
                ensure!(
                    !meta.file_type().is_symlink(),
                    "cache path contains a symlink"
                );
                let owner = unsafe { libc::geteuid() };
                ensure!(
                    (meta.uid() == 0 || meta.uid() == owner)
                        && (meta.mode() & 0o022 == 0
                            || (meta.uid() == 0 && meta.mode() & 0o1000 != 0)),
                    "cache ancestor is app-writable"
                );
            }
        }
        if !path.exists() {
            fs::create_dir_all(path)?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        }
        let meta = fs::symlink_metadata(path)?;
        // SAFETY: geteuid has no preconditions.
        ensure!(
            meta.is_dir() && meta.uid() == unsafe { libc::geteuid() } && meta.mode() & 0o022 == 0,
            "cache must be owned by the loader and not group/world writable"
        );
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path.join(".lock"))?;
        lock.lock_exclusive()?;
        for directory in ["blobs", "resolutions"] {
            let target = path.join(directory);
            if !target.exists() {
                fs::create_dir(&target)?;
            }
            let meta = fs::symlink_metadata(&target)?;
            ensure!(
                meta.is_dir()
                    && !meta.file_type().is_symlink()
                    && meta.uid() == unsafe { libc::geteuid() }
                    && meta.mode() & 0o022 == 0,
                "unsafe cache directory"
            );
        }
        Ok(Self {
            path: path.into(),
            _lock: lock,
        })
    }
    pub fn blob(&self, digest: &str) -> Result<PathBuf> {
        Ok(self.path.join("blobs").join(digest_name(digest)?))
    }
    pub fn resolution(&self, reference: &str) -> PathBuf {
        self.path
            .join("resolutions")
            .join(crate::digest(reference.as_bytes()).replace(':', "-"))
    }
    pub fn write(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
        temporary.write_all(bytes)?;
        temporary.as_file().sync_all()?;
        temporary.persist(path)?;
        Ok(())
    }
}
