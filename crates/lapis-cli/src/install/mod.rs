//! Fixed per-user destinations. No renderer-provided installation paths.
mod receipt;
mod shell;
pub use shell::Shell;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    NotInstalled,
    Installed,
    Broken,
    Conflict,
}

pub fn destination(home: &Path) -> PathBuf {
    home.join(".local/bin/lapis")
}
fn managed(home: &Path) -> PathBuf {
    home.join(".local/share/lapis/cli/lapis")
}
fn conflict() -> io::Error {
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        "An unrelated command or symlink occupies the Lapis installation path",
    )
}

/// Refuse symlink ancestors, including dangling links, before any mutation.
fn directories(home: &Path, relative: &str) -> io::Result<PathBuf> {
    if !home.is_absolute() {
        return Err(io::Error::other("Home directory must be absolute"));
    }
    let mut path = home.to_path_buf();
    for part in relative.split('/') {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(m) if m.is_dir() && !m.is_symlink() => {}
            Ok(_) => return Err(conflict()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => fs::create_dir(&path)?,
            Err(e) => return Err(e),
        }
    }
    Ok(path)
}

pub fn status(home: &Path) -> io::Result<Status> {
    let owned = receipt::load(home)?;
    let binary = managed(home);
    let file = fs::symlink_metadata(&binary);
    let binary_exists = match file {
        Ok(m) if m.is_file() && !m.is_symlink() => {
            if owned
                .as_ref()
                .is_none_or(|r| receipt::digest(&binary).ok().as_deref() != Some(r.sha256.as_str()))
            {
                return Ok(Status::Conflict);
            }
            true
        }
        Ok(_) => return Ok(Status::Conflict),
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => return Err(e),
    };
    match fs::symlink_metadata(destination(home)) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(if owned.is_some() {
            Status::Broken
        } else {
            Status::NotInstalled
        }),
        Err(e) => Err(e),
        Ok(m)
            if m.is_symlink() && fs::read_link(destination(home))? == binary && owned.is_some() =>
        {
            Ok(if binary_exists {
                Status::Installed
            } else {
                Status::Broken
            })
        }
        Ok(_) => Ok(Status::Conflict),
    }
}

/// Install a private copy so app relocation cannot strand the command.
#[cfg(unix)]
pub fn install(
    home: &Path,
    source: &Path,
    shell: Option<Shell>,
    replace: bool,
) -> io::Result<Status> {
    use std::os::unix::fs::{PermissionsExt, symlink};
    directories(home, ".local/bin")?;
    directories(home, ".local/share/lapis/cli")?;
    let _lease = receipt::lease(home)?;
    let state = status(home)?;
    if state == Status::Conflict {
        return Err(conflict());
    }
    let target = managed(home);
    let previous = receipt::load(home)?;
    if !replace && source != target && previous.as_ref().is_some_and(|r| r.source != source) {
        return Err(io::Error::other(
            "A different Lapis installation owns this command. Use --replace to switch sources explicitly.",
        ));
    }
    if let Ok(m) = fs::symlink_metadata(&target)
        && (m.is_symlink() || !m.is_file() || (state == Status::NotInstalled && previous.is_none()))
    {
        return Err(conflict());
    }
    let source_metadata = fs::metadata(source)?;
    if !source.is_absolute()
        || !source_metadata.is_file()
        || source_metadata.permissions().mode() & 0o111 == 0
    {
        return Err(io::Error::other(
            "Bundled CLI must be an absolute executable file",
        ));
    }
    let temporary = target.with_file_name(format!(".lapis-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        output.set_permissions(fs::Permissions::from_mode(0o700))?;
        io::copy(&mut fs::File::open(source)?, &mut output)?;
        output.sync_all()?;
        fs::rename(&temporary, &target)?;
        let origin = previous
            .as_ref()
            .filter(|_| source == target)
            .map(|r| r.source.as_path())
            .unwrap_or(source);
        receipt::write(home, origin, &target)?;
        if fs::symlink_metadata(destination(home)).is_err() {
            symlink(&target, destination(home))?;
        }
        if let Some(shell) = shell {
            shell::setup(home, shell)?;
        }
        status(home)
    })();
    let _ = fs::remove_file(&temporary);
    result
}

#[cfg(not(unix))]
pub fn install(_: &Path, _: &Path, _: Option<Shell>, _: bool) -> io::Result<Status> {
    Err(io::Error::other(
        "Automatic installation supports macOS and Linux",
    ))
}

pub fn uninstall(home: &Path, remove_path: bool) -> io::Result<()> {
    match status(home)? {
        Status::Conflict => return Err(conflict()),
        Status::NotInstalled => {}
        _ => {
            // Validate the fixed parents before unlinking either entry.
            directories(home, ".local/bin")?;
            directories(home, ".local/share/lapis/cli")?;
            let _lease = receipt::lease(home)?;
            if status(home)? == Status::Conflict {
                return Err(conflict());
            }
            let target = managed(home);
            match fs::symlink_metadata(&target) {
                Ok(m) if m.is_file() && !m.is_symlink() => fs::remove_file(target)?,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                _ => return Err(conflict()),
            }
            match fs::remove_file(destination(home)) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                result => result?,
            }
            fs::remove_file(receipt::path(home))?;
        }
    }
    if remove_path {
        shell::remove(home)?;
    }
    Ok(())
}
