use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Receipt {
    pub source: PathBuf,
    pub sha256: String,
    pub version: String,
}
pub(super) fn path(home: &Path) -> PathBuf {
    home.join(".local/share/lapis/cli/installation.json")
}
pub(super) fn digest(path: &Path) -> io::Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}
pub(super) fn load(home: &Path) -> io::Result<Option<Receipt>> {
    let path = path(home);
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
        Ok(m) if m.is_symlink() || !m.is_file() || m.len() > 32 * 1024 => {
            return Err(super::conflict());
        }
        _ => {}
    }
    let receipt: Receipt =
        serde_json::from_slice(&fs::read(path)?).map_err(|_| super::conflict())?;
    if !receipt.source.is_absolute()
        || receipt.sha256.len() != 64
        || !receipt.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(super::conflict());
    }
    Ok(Some(receipt))
}
pub(super) fn write(home: &Path, source: &Path, binary: &Path) -> io::Result<()> {
    use io::Write;
    let receipt = Receipt {
        source: source.to_path_buf(),
        sha256: digest(binary)?,
        version: env!("CARGO_PKG_VERSION").into(),
    };
    let path = path(home);
    let temporary = path.with_extension(format!("{}", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(&serde_json::to_vec(&receipt)?)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    let _ = fs::remove_file(temporary);
    result
}

pub(super) fn lease(home: &Path) -> io::Result<fs::File> {
    let path = path(home).with_file_name("installation.lock");
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(super::conflict());
    }
    file.try_lock().map_err(|_| {
        io::Error::new(
            io::ErrorKind::WouldBlock,
            "Another CLI setup is running; retry when it finishes",
        )
    })?;
    Ok(file)
}
