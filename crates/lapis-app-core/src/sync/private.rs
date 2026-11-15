//! Profile capabilities remain authoritative throughout private-store admission.
use super::{invalid, refused, validate_id};
use crate::vault::sync_directory;
use cap_std::fs::Dir;
use same_file::Handle;
use std::{
    io,
    path::{Path, PathBuf},
};

pub(super) fn open_private_leaf(
    profile: &Path,
    vault_id: &str,
    destination_id: &str,
    leaf: &'static str,
) -> io::Result<(PathBuf, Handle, Dir, PathBuf)> {
    validate_id(vault_id)?;
    validate_id(destination_id)?;
    if !profile.is_absolute() {
        return Err(invalid("Sync profile must be absolute"));
    }
    let metadata = std::fs::symlink_metadata(profile)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(refused("Sync profile must be a regular directory"));
    }
    let admitted = crate::vault::VaultRoot::open(profile)?;
    use std::os::unix::fs::MetadataExt as _;
    if admitted.physical_identity()? != (metadata.dev(), metadata.ino()) {
        return Err(refused("Sync profile changed during admission"));
    }
    open_private_leaf_admitted(&admitted, vault_id, destination_id, leaf)
}

pub(super) fn open_private_leaf_admitted(
    profile: &crate::vault::VaultRoot,
    vault_id: &str,
    destination_id: &str,
    leaf: &'static str,
) -> io::Result<(PathBuf, Handle, Dir, PathBuf)> {
    validate_id(vault_id)?;
    validate_id(destination_id)?;
    profile.verify_physical_root()?;
    let profile_identity = Handle::from_file(profile.dir.try_clone()?.into_std_file())?;
    let components = [
        "sync",
        "v1",
        "vaults",
        vault_id,
        "destinations",
        destination_id,
        leaf,
    ];
    let mut directory = profile.dir.try_clone()?;
    let mut path = profile.path().to_path_buf();
    for component in components {
        directory = private_child(&directory, component)?;
        path.push(component);
    }
    profile.verify_physical_root()?;
    Ok((
        profile.path().to_path_buf(),
        profile_identity,
        directory,
        path,
    ))
}

fn private_child(parent: &Dir, name: &str) -> io::Result<Dir> {
    let mut created = false;
    match parent.symlink_metadata(name) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(refused("Sync repository components must be directories"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => match parent.create_dir(name) {
            Ok(()) => created = true,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        },
        Err(error) => return Err(error),
    }
    let metadata = parent.symlink_metadata(name)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(refused("Sync repository components must be directories"));
    }
    #[cfg(unix)]
    {
        use cap_std::fs::PermissionsExt;
        parent.set_permissions(name, cap_std::fs::Permissions::from_mode(0o700))?;
    }
    if created {
        sync_directory(parent)?;
    }
    let child = parent.open_dir(name)?;
    verify_opened_child(&child, &parent.symlink_metadata(name)?)?;
    Ok(child)
}

#[cfg(unix)]
fn verify_opened_child(child: &Dir, expected: &cap_std::fs::Metadata) -> io::Result<()> {
    use cap_std::fs::MetadataExt;
    let actual = child.dir_metadata()?;
    if actual.dev() != expected.dev() || actual.ino() != expected.ino() {
        return Err(refused(
            "Sync repository component changed during admission",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_opened_child(child: &Dir, _: &cap_std::fs::Metadata) -> io::Result<()> {
    if !child.dir_metadata()?.is_dir() {
        return Err(refused(
            "Sync repository component changed during admission",
        ));
    }
    Ok(())
}
